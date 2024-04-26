use std::io::{BufRead, BufReader};
use std::time::{Duration, Instant};

use actix_web::web::{Bytes, Data};
use anyhow::Context;
use ethers::abi::{encode_packed, Token};
use ethers::types::U256;
use ethers::utils::keccak256;
use k256::ecdsa::SigningKey;
use k256::elliptic_curve::generic_array::sequence::Lengthen;
use tokio::sync::mpsc::Sender;
use tokio::time::timeout;

use crate::utils::{AppState, ExecutionResponse, JobResponse};
use crate::workerd;
use crate::workerd::ServerlessError::*;

/* Error code semantics:-
1 => Provided txn hash doesn't belong to the expected rpc chain or code contract
2 => Calldata corresponding to the txn hash is invalid
3 => Syntax error in the code extracted from the calldata
4 => User timeout exceeded */

// Execute the job request using workerd runtime and 'cgroup' environment
pub async fn execute_job(
    job_id: U256,
    code_hash: String,
    code_inputs: Bytes,
    user_deadline: u64,
    app_state: Data<AppState>,
    tx: Sender<JobResponse>,
) {
    let execution_timer_start = Instant::now();

    let slug = &hex::encode(rand::random::<u32>().to_ne_bytes());

    // Create the code file in the desired location
    if let Err(err) = workerd::create_code_file(
        &code_hash,
        slug,
        &app_state.workerd_runtime_path,
        &app_state.http_rpc_url,
        &app_state.code_contract_addr,
    )
    .await
    {
        let execution_total_time = total_time_passed(execution_timer_start);

        return match err {
            TxNotFound | InvalidTxToType | InvalidTxToValue(_, _) => {
                let Some(signature) = sign_response(
                    &app_state.enclave_signer_key,
                    job_id,
                    Bytes::new(),
                    execution_total_time,
                    1,
                ) else {
                    return;
                };
                if let Err(err) = tx
                    .send(JobResponse {
                        execution_response: Some(ExecutionResponse {
                            id: job_id,
                            output: Bytes::new(),
                            error_code: 1,
                            total_time: execution_total_time,
                            signature: signature.into(),
                        }),
                        timeout_response: None,
                    })
                    .await
                {
                    eprintln!(
                        "Failed to send execution response to transaction sender: {:?}",
                        err
                    );
                }

                ()
            }
            InvalidTxCalldataType | BadCalldata(_) => {
                let Some(signature) = sign_response(
                    &app_state.enclave_signer_key,
                    job_id,
                    Bytes::new(),
                    execution_total_time,
                    2,
                ) else {
                    return;
                };
                if let Err(err) = tx
                    .send(JobResponse {
                        execution_response: Some(ExecutionResponse {
                            id: job_id,
                            output: Bytes::new(),
                            error_code: 2,
                            total_time: execution_total_time,
                            signature: signature.into(),
                        }),
                        timeout_response: None,
                    })
                    .await
                {
                    eprintln!(
                        "Failed to send execution response to transaction sender: {:?}",
                        err
                    );
                }

                ()
            }
            _ => (),
        };
    }

    // Reserve a 'cgroup' for code execution
    let cgroup = app_state.cgroups.lock().unwrap().reserve();
    let Ok(cgroup) = cgroup else {
        let _ = workerd::cleanup_code_file(&code_hash, slug, &app_state.workerd_runtime_path).await;

        eprintln!("No free cgroup available to execute the job");
        return;
    };

    // Get free port for the 'cgroup'
    let Ok(port) = workerd::get_port(&cgroup) else {
        app_state.cgroups.lock().unwrap().release(cgroup);
        let _ = workerd::cleanup_code_file(&code_hash, slug, &app_state.workerd_runtime_path).await;

        return;
    };

    // Create config file in the desired location
    if let Err(_) =
        workerd::create_config_file(&code_hash, slug, &app_state.workerd_runtime_path, port).await
    {
        app_state.cgroups.lock().unwrap().release(cgroup);
        let _ = workerd::cleanup_code_file(&code_hash, slug, &app_state.workerd_runtime_path).await;

        return;
    }

    // Start workerd execution on the user code file using the config file
    let child = workerd::execute(&code_hash, slug, &app_state.workerd_runtime_path, &cgroup).await;
    let Ok(mut child) = child else {
        let _ =
            workerd::cleanup_config_file(&code_hash, slug, &app_state.workerd_runtime_path).await;

        app_state.cgroups.lock().unwrap().release(cgroup);
        let _ = workerd::cleanup_code_file(&code_hash, slug, &app_state.workerd_runtime_path).await;

        return;
    };

    // Wait for worker to be available to receive inputs
    let res = workerd::wait_for_port(port).await;

    if !res {
        // Kill the worker
        child
            .kill()
            .context("CRITICAL: Failed to kill worker {cgroup}")
            .unwrap_or_else(|err| println!("{err:?}"));

        let _ =
            workerd::cleanup_config_file(&code_hash, slug, &app_state.workerd_runtime_path).await;
        app_state.cgroups.lock().unwrap().release(cgroup);
        let _ = workerd::cleanup_code_file(&code_hash, slug, &app_state.workerd_runtime_path).await;

        let execution_total_time = total_time_passed(execution_timer_start);
        let stderr = child.stderr.take().unwrap();
        let reader = BufReader::new(stderr);
        let stderr_lines: Vec<String> = reader.lines().map(|l| l.unwrap()).collect();
        let stderr_output = stderr_lines.join("\n");

        // Check if there was a syntax error in the user code
        if stderr_output != "" && stderr_output.contains("SyntaxError") {
            let Some(signature) = sign_response(
                &app_state.enclave_signer_key,
                job_id,
                Bytes::new(),
                execution_total_time,
                3,
            ) else {
                return;
            };
            if let Err(err) = tx
                .send(JobResponse {
                    execution_response: Some(ExecutionResponse {
                        id: job_id,
                        output: Bytes::new(),
                        error_code: 3,
                        total_time: execution_total_time,
                        signature: signature.into(),
                    }),
                    timeout_response: None,
                })
                .await
            {
                eprintln!(
                    "Failed to send execution response to transaction sender: {:?}",
                    err
                );
            }

            return;
        }

        eprintln!("Failed to execute worker service to serve the user code: {stderr_output}");
        return;
    }

    // Worker is ready, Make the request with the expected user timeout
    let response = timeout(
        Duration::from_secs(user_deadline),
        workerd::get_workerd_response(port, code_inputs),
    )
    .await;

    // Kill the worker
    child
        .kill()
        .context("CRITICAL: Failed to kill worker {cgroup}")
        .unwrap_or_else(|err| println!("{err:?}"));
    let _ = workerd::cleanup_config_file(&code_hash, slug, &app_state.workerd_runtime_path).await;
    app_state.cgroups.lock().unwrap().release(cgroup);
    let _ = workerd::cleanup_code_file(&code_hash, slug, &app_state.workerd_runtime_path).await;

    let execution_total_time = total_time_passed(execution_timer_start);
    let Ok(response) = response else {
        let Some(signature) = sign_response(
            &app_state.enclave_signer_key,
            job_id,
            Bytes::new(),
            execution_total_time,
            4,
        ) else {
            return;
        };
        if let Err(err) = tx
            .send(JobResponse {
                execution_response: Some(ExecutionResponse {
                    id: job_id,
                    output: Bytes::new(),
                    error_code: 4,
                    total_time: execution_total_time,
                    signature: signature.into(),
                }),
                timeout_response: None,
            })
            .await
        {
            eprintln!(
                "Failed to send execution response to transaction sender: {:?}",
                err
            );
        }

        return;
    };

    let Ok(response) = response else {
        return;
    };

    let Some(signature) = sign_response(
        &app_state.enclave_signer_key,
        job_id,
        response.clone(),
        execution_total_time,
        0,
    ) else {
        return;
    };
    if let Err(err) = tx
        .send(JobResponse {
            execution_response: Some(ExecutionResponse {
                id: job_id,
                output: response,
                error_code: 0,
                total_time: execution_total_time,
                signature: signature.into(),
            }),
            timeout_response: None,
        })
        .await
    {
        eprintln!(
            "Failed to send execution response to transaction sender: {:?}",
            err
        );
    }

    ()
}

// Sign the execution response with the enclave key to be verified by the jobs contract
fn sign_response(
    signer_key: &SigningKey,
    job_id: U256,
    output: Bytes,
    total_time: u128,
    error_code: u8,
) -> Option<Vec<u8>> {
    let token_list = [
        Token::Array(vec![Token::Uint(job_id)]),
        Token::Bytes(output.to_owned().into()),
        Token::Array(vec![Token::Uint(total_time.into())]),
        Token::FixedBytes(vec![error_code]),
    ];
    // Encode pack the response details to prepare prehash
    let hash = keccak256(encode_packed(&token_list).unwrap());
    let Ok((rs, v)) = signer_key.sign_prehash_recoverable(&hash).map_err(|err| {
        eprintln!("Failed to sign the response: {:?}", err);
        err
    }) else {
        return None;
    };

    Some(rs.to_bytes().append(27 + v.to_byte()).to_vec())
}

// Calculate and return the time passed in millis since the job execution started
fn total_time_passed(start_time: Instant) -> u128 {
    let execution_timer_end = Instant::now();
    execution_timer_end.duration_since(start_time).as_millis()
}
