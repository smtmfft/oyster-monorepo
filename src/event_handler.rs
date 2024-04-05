use actix_web::web::Data;
use ethers::abi::{decode, ParamType};
use ethers::providers::{Middleware, StreamExt};
use ethers::types::{BigEndianHash, Filter};
use ethers::utils::keccak256;
use tokio::sync::mpsc::{channel, Receiver, Sender};

use crate::job_handler::execute_job;
use crate::timeout_handler::handle_timeout;
use crate::utils::{
    pub_key_to_address, AppState, CommonChainJobs, HttpSignerProvider, JobResponse,
};

pub async fn run_job_listener_channel(app_state: Data<AppState>) {
    let (tx, rx) = channel::<JobResponse>(100);
    let contract_object = app_state
        .jobs_contract_object
        .lock()
        .unwrap()
        .clone()
        .unwrap();
    tokio::spawn(async move {
        send_execution_output(contract_object, rx).await;
    });
    handle_job_relayed(app_state.clone(), tx).await;
}

async fn send_execution_output(
    contract_object: CommonChainJobs<HttpSignerProvider>,
    mut rx: Receiver<JobResponse>,
) {
    while let Some(job_response) = rx.recv().await {
        let Some(execution_response) = job_response.execution_response else {
            if job_response.timeout_response.is_none() {
                continue;
            }

            let txn =
                contract_object.slash_on_execution_timeout(job_response.timeout_response.unwrap());

            let pending_txn = txn.send().await;
            let Ok(pending_txn) = pending_txn else {
                eprintln!(
                    "Failed to send the execution timeout transaction: {}",
                    pending_txn.unwrap_err()
                );
                return;
            };

            let txn_hash = pending_txn.tx_hash();
            let Ok(Some(_)) = pending_txn.confirmations(1).await else {
                // TODO: FIX CONFIRMATIONS REQUIRED
                eprintln!(
                    "Failed to confirm transaction {} for submitting execution timeout",
                    txn_hash
                );
                return;
            };

            return;
        };

        let txn = contract_object.submit_output(
            execution_response.signature.into(),
            execution_response.id,
            execution_response.output.into(),
            execution_response.total_time.into(),
            execution_response.error_code.into(),
        );

        let pending_txn = txn.send().await;
        let Ok(pending_txn) = pending_txn else {
            eprintln!(
                "Failed to send the execution output transaction: {}",
                pending_txn.unwrap_err()
            );
            return;
        };

        let txn_hash = pending_txn.tx_hash();
        let Ok(Some(_)) = pending_txn.confirmations(1).await else {
            // TODO: FIX CONFIRMATIONS REQUIRED
            eprintln!(
                "Failed to confirm transaction {} for submitting execution output",
                txn_hash
            );
            return;
        };
    }
}

async fn handle_job_relayed(app_state: Data<AppState>, tx: Sender<JobResponse>) {
    let event_filter = Filter::new()
        .address(app_state.jobs_contract_addr)
        .topic0(vec![
            keccak256(
                "JobRelayed(uint256,uint256,bytes32,bytes,uint256,address,address,address[])",
            ),
            keccak256("JobResponded(uint256,bytes,uint256,uint256,uint8)"),
            keccak256("ExecutorDeregistered(bytes)"),
        ]);

    let stream = app_state
        .web_socket_client
        .subscribe_logs(&event_filter)
        .await;

    let Ok(mut stream) = stream else {
        eprint!("Failed to subscribe to common chain contract event logs");
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
                    ParamType::Uint(256),
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

            let Some(code_hash) = event_tokens[1].clone().into_fixed_bytes() else {
                eprintln!(
                    "Failed to decode codeHash token from the job relayed event data: {}",
                    event_tokens[2]
                );
                continue;
            };
            let Some(code_inputs) = event_tokens[2].clone().into_bytes() else {
                eprintln!(
                    "Failed to decode codeInputs token from the job relayed event data: {}",
                    event_tokens[3]
                );
                continue;
            };
            let Some(user_deadline) = event_tokens[3].clone().into_uint() else {
                eprintln!(
                    "Failed to decode deadline token from the job relayed event data: {}",
                    event_tokens[4]
                );
                continue;
            };
            let Some(selected_nodes) = event_tokens[6].clone().into_array() else {
                eprintln!(
                    "Failed to decode selectedNodes token from the job relayed event data: {}",
                    event_tokens[5]
                );
                continue;
            };

            let current_node =
                pub_key_to_address(app_state.enclave_pub_key.lock().unwrap().as_ref());
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

            app_state
                .job_requests_running
                .lock()
                .unwrap()
                .insert(job_id);

            let app_state_1 = app_state.clone();
            let tx_1 = tx.clone();
            tokio::spawn(async move {
                handle_timeout(job_id, user_deadline.as_u64(), app_state_1, tx_1).await;
            });

            if is_node_selected {
                let code_hash =
                    String::from("0x".to_owned() + &data_encoding::HEXLOWER.encode(&code_hash));

                let app_state_2 = app_state.clone();
                let tx_2 = tx.clone();
                tokio::spawn(async move {
                    execute_job(
                        job_id,
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
            == keccak256("JobResponded(uint256,bytes,uint256,uint256,uint8)").into()
        {
            let job_id = event.topics[1].into_uint();

            app_state
                .job_requests_running
                .lock()
                .unwrap()
                .remove(&job_id);
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

            if *app_state.enclave_pub_key.lock().unwrap() == enclave_pub_key {
                *app_state.registered.lock().unwrap() = false;
            }
        }
    }

    return;
}
