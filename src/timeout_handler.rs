use actix_web::web::Data;
use ethers::types::U256;
use tokio::sync::mpsc::Sender;
use tokio::time::{sleep, Duration};

use crate::utils::{AppState, JobResponse};

pub async fn handle_timeout(
    job_id: U256,
    req_chain_id: U256,
    job_key: U256,
    timeout: u64,
    app_state: Data<AppState>,
    tx: Sender<JobResponse>,
) {
    sleep(Duration::from_secs(
        timeout + app_state.execution_buffer_time,
    ))
    .await;

    if !app_state
        .job_requests_running
        .lock()
        .unwrap()
        .contains(&job_key)
    {
        return;
    }

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

    app_state
        .job_requests_running
        .lock()
        .unwrap()
        .remove(&job_key);
}
