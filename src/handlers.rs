use alloy::rpc::types::Log;
use anyhow::Result;
use tracing::{info, instrument};

#[instrument(level = "info", skip_all, parent = None, fields(block = log.block_number, idx = log.log_index))]
pub fn handle_log(log: Log) -> Result<()> {
    info!(?log, "processing");
    Ok(())
}
