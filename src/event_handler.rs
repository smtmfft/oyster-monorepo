use actix_web::web::Data;
use ethers::abi::{decode, ParamType};
use ethers::providers::{Middleware, StreamExt};
use ethers::types::{BigEndianHash, Filter};
use ethers::utils::keccak256;
use tokio::sync::mpsc::{channel, Receiver, Sender};

use crate::job_handler::execute_job;
use crate::timeout_handler::handle_timeout;
use crate::utils::{
    get_job_key, pub_key_to_address, send_txn, AppState, CommonChainJobs, HttpSignerProvider,
    JobResponse,
};

pub async fn run_job_listener_channel(app_state: Data<AppState>) {
    let jobs_event_filter = Filter::new()
        .address(app_state.jobs_contract_addr)
        .topic0(vec![
            keccak256(
                "JobRelayed(uint256,uint256,bytes32,bytes,uint256,address,address,address[])",
            ),
            keccak256("JobResponded(uint256,uint256,bytes,uint256,uint256,uint8)"),
        ]);

    let executors_event_filter = Filter::new()
        .address(app_state.executors_contract_addr)
        .topic0(vec![keccak256("ExecutorDeregistered(bytes)")]);

    let Some(jobs_contract_object) = app_state.jobs_contract_object.lock().unwrap().clone() else {
        eprintln!("CommonChainJobs contract object not found!");
        return;
    };

    let (tx, rx) = channel::<JobResponse>(100);
    let app_state_clone = app_state.clone();
    let tx_clone = tx.clone();

    tokio::spawn(async move {
        send_execution_output(jobs_contract_object, rx).await;
    });
    tokio::spawn(async move {
        handle_event_logs(executors_event_filter, app_state_clone, tx_clone).await;
    });
    handle_event_logs(jobs_event_filter, app_state, tx).await;
}

async fn send_execution_output(
    contract_object: CommonChainJobs<HttpSignerProvider>,
    mut rx: Receiver<JobResponse>,
) {
    while let Some(job_response) = rx.recv().await {
        let Some(execution_response) = job_response.execution_response else {
            let Some((job_id, req_chain_id)) = job_response.timeout_response else {
                continue;
            };
            let txn = contract_object.slash_on_execution_timeout(job_id, req_chain_id);

            let txn_result = send_txn(txn).await;
            let Ok(_) = txn_result else {
                eprintln!(
                    "Failed to submit the execution timeout transaction: {}",
                    txn_result.unwrap_err()
                );
                continue;
            };

            continue;
        };

        let txn = contract_object.submit_output(
            execution_response.signature.into(),
            execution_response.id,
            execution_response.req_chain_id,
            execution_response.output.into(),
            execution_response.total_time.into(),
            execution_response.error_code.into(),
        );

        let txn_result = send_txn(txn).await;
        let Ok(_) = txn_result else {
            eprintln!(
                "Failed to submit the execution output transaction: {}",
                txn_result.unwrap_err()
            );
            continue;
        };
    }
}

async fn handle_event_logs(filter: Filter, app_state: Data<AppState>, tx: Sender<JobResponse>) {
    let stream = app_state.web_socket_client.subscribe_logs(&filter).await;

    let Ok(mut stream) = stream else {
        eprintln!(
            "Failed to subscribe to contract {:?} event logs",
            filter.address
        );
        return;
    };

    while let Some(event) = stream.next().await {
        if *app_state.registered.lock().unwrap() == false {
            eprintln!("Enclave deregistered!");
            return;
        }

        if event.removed.unwrap_or(false) {
            continue;
        }

        if event.topics[0]
            == keccak256(
                "JobRelayed(uint256,uint256,bytes32,bytes,uint256,address,address,address[])",
            )
            .into()
        {
            let event_tokens = decode(
                &vec![
                    ParamType::FixedBytes(32),
                    ParamType::Bytes,
                    ParamType::Uint(256),
                    ParamType::Address,
                    ParamType::Address,
                    ParamType::Array(Box::new(ParamType::Address)),
                ],
                &event.data.to_vec(),
            );
            let Ok(event_tokens) = event_tokens else {
                eprintln!(
                    "Failed to decode job relayed event data {}: {}",
                    event.data,
                    event_tokens.unwrap_err()
                );
                continue;
            };

            let job_id = event.topics[1].into_uint();
            let req_chain_id = event.topics[2].into_uint();

            let Some(code_hash) = event_tokens[0].clone().into_fixed_bytes() else {
                eprintln!(
                    "Failed to decode codeHash token from the job relayed event data: {}",
                    event_tokens[0]
                );
                continue;
            };
            let Some(code_inputs) = event_tokens[1].clone().into_bytes() else {
                eprintln!(
                    "Failed to decode codeInputs token from the job relayed event data: {}",
                    event_tokens[1]
                );
                continue;
            };
            let Some(user_deadline) = event_tokens[2].clone().into_uint() else {
                eprintln!(
                    "Failed to decode deadline token from the job relayed event data: {}",
                    event_tokens[2]
                );
                continue;
            };
            let Some(selected_nodes) = event_tokens[5].clone().into_array() else {
                eprintln!(
                    "Failed to decode selectedNodes token from the job relayed event data: {}",
                    event_tokens[5]
                );
                continue;
            };

            let current_node = pub_key_to_address(app_state.enclave_pub_key.as_ref());
            let Ok(current_node) = current_node else {
                eprintln!(
                    "Failed to parse the enclave public key into eth address: {}",
                    current_node.unwrap_err()
                );
                continue;
            };

            let is_node_selected = selected_nodes
                .into_iter()
                .map(|token| token.into_address())
                .filter(|addr| addr.is_some())
                .any(|addr| addr.unwrap() == current_node);

            let job_key = get_job_key(job_id, req_chain_id);
            let Ok(job_key) = job_key else {
                eprintln!(
                    "Failed to extract job key from job ID and req chain ID: {}",
                    job_key.unwrap_err()
                );
                continue;
            };

            app_state
                .job_requests_running
                .lock()
                .unwrap()
                .insert(job_key);

            let app_state_1 = app_state.clone();
            let tx_1 = tx.clone();
            tokio::spawn(async move {
                handle_timeout(
                    job_id,
                    req_chain_id,
                    job_key,
                    user_deadline.as_u64(),
                    app_state_1,
                    tx_1,
                )
                .await;
            });

            if is_node_selected {
                let code_hash =
                    String::from("0x".to_owned() + &data_encoding::HEXLOWER.encode(&code_hash));

                let app_state_2 = app_state.clone();
                let tx_2 = tx.clone();
                tokio::spawn(async move {
                    execute_job(
                        job_id,
                        req_chain_id,
                        code_hash,
                        code_inputs.into(),
                        user_deadline.as_u64(),
                        app_state_2,
                        tx_2,
                    )
                    .await;
                });
            }
        } else if event.topics[0]
            == keccak256("JobResponded(uint256,uint256,bytes,uint256,uint256,uint8)").into()
        {
            let job_id = event.topics[1].into_uint();
            let req_chain_id = event.topics[2].into_uint();

            let job_key = get_job_key(job_id, req_chain_id);
            let Ok(job_key) = job_key else {
                eprintln!(
                    "Failed to extract job key from job ID and req chain ID: {}",
                    job_key.unwrap_err()
                );
                continue;
            };

            app_state
                .job_requests_running
                .lock()
                .unwrap()
                .remove(&job_key);
        } else if event.topics[0] == keccak256("ExecutorDeregistered(bytes)").into() {
            let event_tokens = decode(&vec![ParamType::Bytes], &event.data.to_vec());
            let Ok(event_tokens) = event_tokens else {
                eprintln!(
                    "Failed to decode executor deregistered event data {}: {}",
                    event.data,
                    event_tokens.unwrap_err()
                );
                continue;
            };

            let Some(enclave_pub_key) = event_tokens[0].clone().into_bytes() else {
                eprintln!(
                    "Failed to decode enclavePubKey token from the executor deregistered event data: {}",
                    event_tokens[0]
                );
                continue;
            };

            if app_state.enclave_pub_key == enclave_pub_key {
                *app_state.registered.lock().unwrap() = false;
            }
        }
    }
}
