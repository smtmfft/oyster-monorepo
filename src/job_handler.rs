use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use actix_web::web::{Bytes, Data};
use anyhow::Context;
use ethers::abi::{encode, encode_packed, Token};
use ethers::types::U256;
use ethers::utils::keccak256;
use k256::ecdsa::SigningKey;
use k256::elliptic_curve::generic_array::sequence::Lengthen;
use scopeguard::defer;
use tokio::sync::mpsc::Sender;
use tokio::time::timeout;

use crate::utils::{AppState, ExecutionResponse, JobOutput, JobResponse};
use crate::workerd;
use crate::workerd::ServerlessError::*;

/* Error code semantics:-
1 => Provided txn hash doesn't belong to the expected rpc chain or code contract
2 => Syntax error in the code extracted from the calldata
3 => User timeout exceeded */

// Execute the job request using workerd runtime and 'cgroup' environment
pub async fn handle_job(
    job_id: U256,
    code_hash: String,
    code_inputs: Bytes,
    user_deadline: u64, // time in millis
    app_state: Data<AppState>,
    tx: Sender<JobResponse>,
) {
    let slug = &hex::encode(rand::random::<u32>().to_ne_bytes());

    // Execute the job request under the specified user deadline
    let response = timeout(
        Duration::from_millis(user_deadline),
        execute_job(&code_hash, code_inputs, slug, app_state.clone()),
    )
    .await;

    // clean up resources in case the timeout exceeds
    let _ = workerd::cleanup_config_file(&code_hash, slug, &app_state.workerd_runtime_path).await;
    let _ = workerd::cleanup_code_file(&code_hash, slug, &app_state.workerd_runtime_path).await;

    // Initialize the default timeout response and build on that based on the above response
    let mut execution_response = Some(ExecutionResponse {
        output: Bytes::new(),
        error_code: 3,
        total_time: user_deadline.into(),
    });
    if response.is_ok() {
        execution_response = response.unwrap();
    }
    if execution_response.is_none() {
        return;
    }

    // Sign and send the job response to the receiver channel
    let execution_response = execution_response.unwrap();
    let sign_timestamp: U256 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .into();

    let Some(signature) = sign_response(
        &app_state.enclave_signer,
        job_id,
        &execution_response.output,
        execution_response.total_time,
        execution_response.error_code,
        sign_timestamp,
    ) else {
        return;
    };

    if let Err(err) = tx
        .send(JobResponse {
            job_output: Some(JobOutput {
                signature: signature.into(),
                id: job_id,
                execution_response: execution_response,
                sign_timestamp: sign_timestamp,
            }),
            timeout_response: None,
        })
        .await
    {
        eprintln!(
            "Failed to send execution response to receiver channel: {:?}",
            err
        );
    }

    return;
}

async fn execute_job(
    code_hash: &String,
    code_inputs: Bytes,
    slug: &String,
    app_state: Data<AppState>,
) -> Option<ExecutionResponse> {
    let execution_timer_start = Instant::now();

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
        return match err {
            TxNotFound | InvalidTxToType | InvalidTxToValue(_, _) => Some(ExecutionResponse {
                output: Bytes::new(),
                error_code: 1,
                total_time: execution_timer_start.elapsed().as_millis().into(),
            }),
            _ => None,
        };
    }

    // Reserve a 'cgroup' for code execution
    let cgroup = app_state.cgroups.lock().unwrap().reserve();
    let Ok(cgroup) = cgroup else {
        let _ = workerd::cleanup_code_file(&code_hash, slug, &app_state.workerd_runtime_path).await;

        eprintln!("No free cgroup available to execute the job");
        return None;
    };

    // clean up resources in case the timeout exceeds
    let cgroup_clone = cgroup.clone();
    defer! {
        app_state.cgroups.lock().unwrap().release(cgroup_clone);
    }

    // Get free port for the 'cgroup'
    let Ok(port) = workerd::get_port(&cgroup) else {
        app_state.cgroups.lock().unwrap().release(cgroup);
        let _ = workerd::cleanup_code_file(&code_hash, slug, &app_state.workerd_runtime_path).await;

        return None;
    };

    // Create config file in the desired location
    if let Err(_) =
        workerd::create_config_file(&code_hash, slug, &app_state.workerd_runtime_path, port).await
    {
        app_state.cgroups.lock().unwrap().release(cgroup);
        let _ = workerd::cleanup_code_file(&code_hash, slug, &app_state.workerd_runtime_path).await;

        return None;
    }

    // Start workerd execution on the user code file using the config file
    let child = workerd::execute(&code_hash, slug, &app_state.workerd_runtime_path, &cgroup).await;
    let Ok(child) = child else {
        let _ =
            workerd::cleanup_config_file(&code_hash, slug, &app_state.workerd_runtime_path).await;

        app_state.cgroups.lock().unwrap().release(cgroup);
        let _ = workerd::cleanup_code_file(&code_hash, slug, &app_state.workerd_runtime_path).await;

        return None;
    };
    let child = Arc::new(Mutex::new(child));

    // clean up resources in case the timeout exceeds
    defer! {
        // Kill the worker
        child
            .lock()
            .unwrap()
            .kill()
            .context("CRITICAL: Failed to kill worker {cgroup}")
            .unwrap_or_else(|err| println!("{err:?}"));
    }

    // Wait for worker to be available to receive inputs
    let res = workerd::wait_for_port(port).await;

    if !res {
        // Kill the worker
        child
            .lock()
            .unwrap()
            .kill()
            .context("CRITICAL: Failed to kill worker {cgroup}")
            .unwrap_or_else(|err| println!("{err:?}"));

        let _ =
            workerd::cleanup_config_file(&code_hash, slug, &app_state.workerd_runtime_path).await;
        app_state.cgroups.lock().unwrap().release(cgroup);
        let _ = workerd::cleanup_code_file(&code_hash, slug, &app_state.workerd_runtime_path).await;

        let mut child_guard = child.lock().unwrap();
        let Some(stderr) = child_guard.stderr.take() else {
            eprintln!("Failed to retrieve cgroup execution error");
            return None;
        };
        let reader = BufReader::new(stderr);
        let stderr_lines: Vec<String> = reader.lines().map(|l| l.unwrap()).collect();
        let stderr_output = stderr_lines.join("\n");

        // Check if there was a syntax error in the user code
        if stderr_output != "" && stderr_output.contains("SyntaxError") {
            return Some(ExecutionResponse {
                output: Bytes::new(),
                error_code: 2,
                total_time: execution_timer_start.elapsed().as_millis().into(),
            });
        }

        eprintln!("Failed to execute worker service to serve the user code: {stderr_output}");
        return None;
    }

    // Worker is ready, Make the request with the expected user timeout
    let response = workerd::get_workerd_response(port, code_inputs).await;

    // Kill the worker
    child
        .lock()
        .unwrap()
        .kill()
        .context("CRITICAL: Failed to kill worker {cgroup}")
        .unwrap_or_else(|err| println!("{err:?}"));
    let _ = workerd::cleanup_config_file(&code_hash, slug, &app_state.workerd_runtime_path).await;
    app_state.cgroups.lock().unwrap().release(cgroup);
    let _ = workerd::cleanup_code_file(&code_hash, slug, &app_state.workerd_runtime_path).await;

    let Ok(response) = response else {
        return None;
    };

    Some(ExecutionResponse {
        output: response,
        error_code: 0,
        total_time: execution_timer_start.elapsed().as_millis().into(),
    })
}

// Sign the execution response with the enclave key to be verified by the jobs contract
fn sign_response(
    signer_key: &SigningKey,
    job_id: U256,
    output: &Bytes,
    total_time: U256,
    error_code: u8,
    sign_timestamp: U256,
) -> Option<Vec<u8>> {
    // Encode and hash the job response details following EIP712 format
    let domain_separator = keccak256(encode(&[
        Token::FixedBytes(keccak256("EIP712Domain(string name,string version)").to_vec()),
        Token::FixedBytes(keccak256("marlin.oyster.Jobs").to_vec()),
        Token::FixedBytes(keccak256("1").to_vec()),
    ]));
    let submit_output_typehash = keccak256("SubmitOutput(uint256 jobId,bytes output,uint256 totalTime,uint8 errorCode,uint256 signTimestamp)");

    let hash_struct = keccak256(encode(&[
        Token::FixedBytes(submit_output_typehash.to_vec()),
        Token::Uint(job_id),
        Token::FixedBytes(keccak256(output).to_vec()),
        Token::Uint(total_time),
        Token::Uint(error_code.into()),
        Token::Uint(sign_timestamp),
    ]));

    // Create the digest
    let digest = encode_packed(&[
        Token::String("\x19\x01".to_string()),
        Token::FixedBytes(domain_separator.to_vec()),
        Token::FixedBytes(hash_struct.to_vec()),
    ]);
    let Ok(digest) = digest else {
        eprintln!(
            "Failed to encode the job response for signing: {:?}",
            digest.unwrap_err()
        );
        return None;
    };
    let digest = keccak256(digest);

    // Sign the response details using enclave key
    let Ok((rs, v)) = signer_key.sign_prehash_recoverable(&digest).map_err(|err| {
        eprintln!("Failed to sign the job response: {:?}", err);
        err
    }) else {
        return None;
    };

    Some(rs.to_bytes().append(27 + v.to_byte()).to_vec())
}
