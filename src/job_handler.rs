use std::io::{BufRead, BufReader};
use std::time::{Duration, Instant};

use actix_web::web::{Bytes, Data};
use anyhow::Context;
use ethers::abi::{encode, Token};
use ethers::types::U256;
use ethers::utils::keccak256;
use k256::ecdsa::SigningKey;
use k256::elliptic_curve::generic_array::sequence::Lengthen;
use tokio::sync::mpsc::Sender;
use tokio::time::timeout;

use crate::utils::{AppState, ExecutionResponse, JobResponse};
use crate::workerd;
use crate::workerd::ServerlessError::*;

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
    let workerd_runtime_path = &app_state.workerd_runtime_path;

    if let Err(err) = workerd::create_code_file(
        &code_hash,
        slug,
        workerd_runtime_path,
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
                        "Failed to send execution response to transaction sender: {}",
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
                        "Failed to send execution response to transaction sender: {}",
                        err
                    );
                }

                ()
            }
            _ => (),
        };
    }

    // reserve cgroup
    let cgroup = app_state.cgroups.lock().unwrap().reserve();
    let Ok(cgroup) = cgroup else {
        // cleanup
        let _ = workerd::cleanup_code_file(&code_hash, slug, workerd_runtime_path).await;

        eprintln!("No free cgroup available to execute the job");
        return;
    };

    // get port for cgroup
    let Ok(port) = workerd::get_port(&cgroup) else {
        // cleanup
        app_state.cgroups.lock().unwrap().release(cgroup);
        let _ = workerd::cleanup_code_file(&code_hash, slug, workerd_runtime_path).await;

        return;
    };

    // create config file
    if let Err(_) = workerd::create_config_file(&code_hash, slug, workerd_runtime_path, port).await
    {
        // cleanup
        app_state.cgroups.lock().unwrap().release(cgroup);
        let _ = workerd::cleanup_code_file(&code_hash, slug, workerd_runtime_path).await;

        return;
    }

    // start worker
    let child = workerd::execute(&code_hash, slug, workerd_runtime_path, &cgroup).await;
    let Ok(mut child) = child else {
        // cleanup
        let _ = workerd::cleanup_config_file(&code_hash, slug, workerd_runtime_path).await;

        app_state.cgroups.lock().unwrap().release(cgroup);
        let _ = workerd::cleanup_code_file(&code_hash, slug, workerd_runtime_path).await;

        return;
    };

    // wait for worker to be available
    let res = workerd::wait_for_port(port).await;

    if !res {
        // cleanup
        child
            .kill()
            .context("CRITICAL: Failed to kill worker {cgroup}")
            .unwrap_or_else(|err| println!("{err:?}"));

        let _ = workerd::cleanup_config_file(&code_hash, slug, workerd_runtime_path).await;
        app_state.cgroups.lock().unwrap().release(cgroup);
        let _ = workerd::cleanup_code_file(&code_hash, slug, workerd_runtime_path).await;

        let execution_total_time = total_time_passed(execution_timer_start);
        let stderr = child.stderr.take().unwrap();
        let reader = BufReader::new(stderr);
        let stderr_lines: Vec<String> = reader.lines().map(|l| l.unwrap()).collect();
        let stderr_output = stderr_lines.join("\n");

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
                    "Failed to send execution response to transaction sender: {}",
                    err
                );
            }

            return;
        }

        eprintln!("Failed to execute worker service to serve the user code: {stderr_output}");
        return;
    }

    // worker is ready, make the request
    let response = timeout(
        Duration::from_secs(user_deadline),
        workerd::get_workerd_response(port, code_inputs),
    )
    .await;

    // cleanup
    child
        .kill()
        .context("CRITICAL: Failed to kill worker {cgroup}")
        .unwrap_or_else(|err| println!("{err:?}"));
    let _ = workerd::cleanup_config_file(&code_hash, slug, workerd_runtime_path).await;
    app_state.cgroups.lock().unwrap().release(cgroup);
    let _ = workerd::cleanup_code_file(&code_hash, slug, workerd_runtime_path).await;

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
                "Failed to send execution response to transaction sender: {}",
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
            "Failed to send execution response to transaction sender: {}",
            err
        );
    }

    ()
}

fn sign_response(
    signer_key: &SigningKey,
    job_id: U256,
    output: Bytes,
    total_time: u128,
    error_code: u8,
) -> Option<String> {
    let hash = keccak256(encode(&[
        Token::Uint(job_id),
        Token::Bytes(output.into()),
        Token::Uint(total_time.into()),
        Token::Uint(error_code.into()),
    ]));
    let Ok((rs, v)) = signer_key.sign_prehash_recoverable(&hash).map_err(|err| {
        eprintln!("Failed to sign the response: {}", err);
        err
    }) else {
        return None;
    };

    Some(hex::encode(rs.to_bytes().append(27 + v.to_byte())))
}

fn total_time_passed(start_time: Instant) -> u128 {
    let execution_timer_end = Instant::now();
    execution_timer_end.duration_since(start_time).as_millis()
}
