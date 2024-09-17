use alloy::rpc::types::Log;
use anyhow::Result;
use diesel::PgConnection;
use tracing::warn;
use tracing::{info, instrument};

#[instrument(level = "info", skip_all, parent = None, fields(block = log.block_number, idx = log.log_index))]
pub fn handle_job_revise_rate_finalized(_conn: &mut PgConnection, log: Log) -> Result<()> {
    info!(?log, "processing");

    // while we do have enough context here to handle this properly,
    // JobClosed makes us handle LockDeleted
    // which also more or less handles the lock aspects of this
    // we still need to set the new rates

    Ok(())
}
