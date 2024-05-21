use std::array::TryFromSliceError;

use actix_web::web::Data;
use ethers::abi::{decode, ParamType};
use ethers::providers::{Middleware, StreamExt};
use ethers::types::{Address, BigEndianHash, Filter, Log};
use ethers::utils::keccak256;
use tokio::select;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio_stream::Stream;

use crate::job_handler::execute_job;
use crate::timeout_handler::handle_timeout;
use crate::utils::{send_txn, AppState, HttpSignerProvider, JobResponse, Jobs};

// Start listening to relevant events emitted by the common chain executors and jobs contract
pub async fn run_job_listener_channel(app_state: Data<AppState>) {
    let jobs_contract_object = Jobs::new(
        app_state.jobs_contract_addr,
        app_state.http_rpc_client.lock().unwrap().clone().unwrap(),
    );

    // Create tokio mpsc channel to receive contract events and send transactions to them
    let (tx, rx) = channel::<JobResponse>(100);

    tokio::spawn(async move {
        send_execution_output(jobs_contract_object, rx).await;
    });

    tokio::spawn(async move {
        // Create filter to listen to specific events emitted by the jobs contract
        let jobs_event_filter = Filter::new()
            .address(app_state.jobs_contract_addr)
            .topic0(vec![
                keccak256("JobRelayed(uint256,bytes32,bytes,uint256,address,address,address[])"),
                keccak256("JobResponded(uint256,bytes,uint256,uint8,uint8)"),
            ]);

        // Subscribe to the jobs filter through the rpc web socket client
        let jobs_stream = app_state
            .web_socket_client
            .subscribe_logs(&jobs_event_filter)
            .await;
        let Ok(jobs_stream) = jobs_stream else {
            eprintln!(
                "Failed to subscribe to jobs contract {:?} event logs",
                app_state.jobs_contract_addr
            );
            return;
        };
        let jobs_stream = std::pin::pin!(jobs_stream);

        // Create filter to listen to specific events emitted by the executors contract
        let executors_event_filter = Filter::new()
            .address(app_state.executors_contract_addr)
            .topic0(vec![keccak256("ExecutorDeregistered(address)")]);

        // Subscribe to the executors filter through the rpc web socket client
        let executors_stream = app_state
            .web_socket_client
            .subscribe_logs(&executors_event_filter)
            .await;
        let Ok(executors_stream) = executors_stream else {
            eprintln!(
                "Failed to subscribe to executors contract {:?} event logs",
                app_state.executors_contract_addr
            );
            return;
        };
        let executors_stream = std::pin::pin!(executors_stream);

        handle_event_logs(jobs_stream, executors_stream, app_state.clone(), tx).await;
    });
}

// Receive job execution responses and send the resulting transactions to the common chain
async fn send_execution_output(
    contract_object: Jobs<HttpSignerProvider>,
    mut rx: Receiver<JobResponse>,
) {
    while let Some(job_response) = rx.recv().await {
        let Some(execution_response) = job_response.execution_response else {
            let Some(job_id) = job_response.timeout_response else {
                continue;
            };

            // Prepare the execution timeout transaction to be send to the jobs contract
            let txn = contract_object.slash_on_execution_timeout(job_id);

            let txn_result = send_txn(txn).await;
            let Ok(_) = txn_result else {
                eprintln!(
                    "Failed to submit the execution timeout transaction: {:?}",
                    txn_result.unwrap_err()
                );
                continue;
            };

            continue;
        };

        // Prepare the execution output transaction to be send to the jobs contract
        let txn = contract_object.submit_output(
            execution_response.signature.into(),
            execution_response.id,
            execution_response.output.into(),
            execution_response.total_time.into(),
            execution_response.error_code.into(),
        );

        let txn_result = send_txn(txn).await;
        let Ok(_) = txn_result else {
            eprintln!(
                "Failed to submit the execution output transaction: {:?}",
                txn_result.unwrap_err()
            );
            continue;
        };
    }

    println!("Transaction sender stopped!");
    return;
}

// Listen to the "jobs" & "executors" contract event logs and process them accordingly
pub async fn handle_event_logs(
    mut jobs_stream: impl Stream<Item = Log> + Unpin,
    mut executors_stream: impl Stream<Item = Log> + Unpin,
    app_state: Data<AppState>,
    tx: Sender<JobResponse>,
) {
    loop {
        select! {
            event = jobs_stream.next() => {
                if let Some(event) = event {
                    // Stop listening to the events if the executor has been deregistered
                    if *app_state.registered.lock().unwrap() == false {
                        println!("Enclave deregistered!");
                        return;
                    }

                    if event.removed.unwrap_or(true) {
                        continue;
                    }

                    // Capture the Job relayed event emitted by the jobs contract
                    if event.topics[0]
                    == keccak256("JobRelayed(uint256,bytes32,bytes,uint256,address,address,address[])")
                    .into()
                    {
                        // Decode the event parameters using the ABI information
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
                                "Failed to decode job relayed event data {}: {:?}",
                                event.data,
                                event_tokens.unwrap_err()
                            );
                            continue;
                        };

                        // Extract the indexed parameters of the event
                        let job_id = event.topics[1].into_uint();

                        let Some(code_hash) = event_tokens[0].clone().into_fixed_bytes() else {
                            eprintln!(
                                "Failed to decode codeHash token from the job relayed event data: {:?}",
                                event_tokens[0]
                            );
                            continue;
                        };
                        let Some(code_inputs) = event_tokens[1].clone().into_bytes() else {
                            eprintln!(
                                "Failed to decode codeInputs token from the job relayed event data: {:?}",
                                event_tokens[1]
                            );
                            continue;
                        };
                        let Some(user_deadline) = event_tokens[2].clone().into_uint() else {
                            eprintln!(
                                "Failed to decode deadline token from the job relayed event data: {:?}",
                                event_tokens[2]
                            );
                            continue;
                        };
                        let Some(selected_nodes) = event_tokens[5].clone().into_array() else {
                            eprintln!(
                                "Failed to decode selectedNodes token from the job relayed event data: {:?}",
                                event_tokens[5]
                            );
                            continue;
                        };

                        let Some(executor_key) = app_state.executor_operator_key.lock().unwrap().clone() else {
                            eprintln!("Executor key not found");
                            continue;
                        };

                        // Check if the executor has been selected for the job execution
                        let is_node_selected = selected_nodes
                            .into_iter()
                            .map(|token| token.into_address())
                            .filter(|addr| addr.is_some())
                            .any(|addr| addr.unwrap() == executor_key);

                        // Mark the current job as under execution
                        app_state
                            .job_requests_running
                            .lock()
                            .unwrap()
                            .insert(job_id);

                        let app_state_clone = app_state.clone();
                        let tx_clone = tx.clone();

                        tokio::spawn(async move {
                            handle_timeout(job_id, user_deadline.as_u64(), app_state_clone, tx_clone).await;
                        });

                        if is_node_selected {
                            let code_hash =
                                String::from("0x".to_owned() + &data_encoding::HEXLOWER.encode(&code_hash));
                            let app_state_clone = app_state.clone();
                            let tx_clone = tx.clone();

                            tokio::spawn(async move {
                                execute_job(
                                    job_id,
                                    code_hash,
                                    code_inputs.into(),
                                    user_deadline.as_u64(),
                                    app_state_clone,
                                    tx_clone,
                                )
                                .await;
                            });
                        }
                    }
                    // Capture the Job responded event emitted by the jobs contract
                    else if event.topics[0]
                    == keccak256("JobResponded(uint256,bytes,uint256,uint8,uint8)").into()
                    {
                        let job_id = event.topics[1].into_uint();

                        // Decode the event parameters using the ABI information
                        let event_tokens = decode(
                            &vec![
                            ParamType::Bytes,
                            ParamType::Uint(256),
                            ParamType::Uint(8),
                            ParamType::Uint(8),
                            ],
                            &event.data.to_vec(),
                        );
                        let Ok(event_tokens) = event_tokens else {
                            eprintln!(
                                "Failed to decode job responded event data {}: {:?}",
                                event.data,
                                event_tokens.unwrap_err()
                            );
                            continue;
                        };

                        let Some(output_count) = event_tokens[3].clone().into_uint() else {
                            eprintln!(
                                "Failed to decode outputCount token from the job responded event data: {:?}",
                                event_tokens[3]
                            );
                            continue;
                        };

                        if output_count == app_state.num_selected_executors.into() {
                            // Mark the job as completed
                            app_state
                                .job_requests_running
                                .lock()
                                .unwrap()
                                .remove(&job_id);
                        }
                    }
                }
            }
            event = executors_stream.next() => {
                if let Some(event) = event {
                    if event.removed.unwrap_or(true) {
                        continue;
                    }

                    // Capture the Executor deregistered event emitted by the executors contract
                    if event.topics[0] == keccak256("ExecutorDeregistered(address)").into() {
                        let Ok(deregistered_executor_bytes): Result<[u8; 20], TryFromSliceError> =
                            event.topics[1].0[12..].try_into().map_err(|err| {
                                eprintln!(
                                    "Failed to parse the executor address from deregistered event: {:?}",
                                    err
                                );
                                err
                            })
                        else {
                            continue;
                        };
                        let deregistered_executor = Address::from_slice(&deregistered_executor_bytes);

                        let Some(executor_key) = app_state.executor_operator_key.lock().unwrap().clone() else {
                            eprintln!("Executor key not found");
                            continue;
                        };

                        // Check if the executor has been deregistered and mark it as deregistered accordingly
                        if executor_key == deregistered_executor {
                            println!("Enclave deregistered!");
                            *app_state.registered.lock().unwrap() = false;

                            println!("Jobs event listening stopped!");
                            return;
                        }
                    }
                }
            }
        }
    }
}
