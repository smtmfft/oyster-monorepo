use actix_web::web::Data;
use ethers::types::U256;
use tokio::sync::mpsc::Sender;
use tokio::time::{sleep, Duration};

use crate::utils::{AppState, JobResponse};

// Start task to handle the execution timeout scenario for a job request
pub async fn handle_timeout(
    job_id: U256,
    req_chain_id: U256,
    job_key: U256,
    timeout: u64,
    app_state: Data<AppState>,
    tx: Sender<JobResponse>,
) {
    sleep(Duration::from_secs(
        timeout + app_state.execution_buffer_time + 1,
    ))
    .await;

    // If the job request had been executed then don't send anything
    if !app_state
        .job_requests_running
        .lock()
        .unwrap()
        .contains(&job_key)
    {
        return;
    }

    // Send job response with timeout counterpart
    if let Err(err) = tx
        .send(JobResponse {
            execution_response: None,
            timeout_response: Some((job_id, req_chain_id)),
        })
        .await
    {
        eprintln!(
            "Failed to send timeout response to transaction sender: {}",
            err
        );
    }

    // Mark the job request as completed from executor side
    app_state
        .job_requests_running
        .lock()
        .unwrap()
        .remove(&job_key);
}
