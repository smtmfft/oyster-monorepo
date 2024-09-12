use alloy::rpc::types::Log;
use anyhow::anyhow;
use anyhow::Result;
use ethp::event;
use tracing::warn;
use tracing::{info, instrument};

static PROVIDER_ADDED_TOPIC: [u8; 32] = event!("ProviderAdded(address,string)");

#[instrument(
    level = "info",
    skip_all,
    parent = None,
    fields(block = log.block_number, idx = log.log_index, tx = ?log.transaction_hash
))]
pub fn handle_log(log: Log) -> Result<()> {
    info!(?log, "processing");

    let log_type = log
        .topic0()
        .ok_or(anyhow!("log does not have topic0, should never happen"))?;

    if log_type == PROVIDER_ADDED_TOPIC {
        handle_provider_added(log)
    } else {
        warn!(?log_type, "unknown log type");
        Ok(())
    }
}

#[instrument(level = "info", skip_all, parent = None, fields(block = log.block_number, idx = log.log_index))]
pub fn handle_provider_added(log: Log) -> Result<()> {
    info!(?log, "processing");
    Ok(())
}
