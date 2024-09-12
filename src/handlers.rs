use alloy::rpc::types::Log;
use anyhow::anyhow;
use anyhow::Result;
use ethp::event;
use tracing::warn;
use tracing::{info, instrument};

static PROVIDER_ADDED_TOPIC: [u8; 32] = event!("ProviderAdded(address,string)");

// Ignored logs
static UPGRADED_TOPIC: [u8; 32] = event!("Upgraded(address)");
static LOCK_WAIT_TIME_UPDATED_TOPIC: [u8; 32] =
    event!("LockWaitTimeUpdated(bytes32,uint256,uint256)");
static ROLE_GRANTED_TOPIC: [u8; 32] = event!("RoleGranted(bytes32,address,address)");
static TOKEN_UPDATED_TOPIC: [u8; 32] = event!("TokenUpdated(address,address)");
static INITIALIZED_TOPIC: [u8; 32] = event!("Initialized(uint8)");

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
    } else if log_type == UPGRADED_TOPIC
        || log_type == LOCK_WAIT_TIME_UPDATED_TOPIC
        || log_type == ROLE_GRANTED_TOPIC
        || log_type == TOKEN_UPDATED_TOPIC
        || log_type == INITIALIZED_TOPIC
    {
        info!(?log_type, "ignoring log type");
        Ok(())
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

// log ProviderAdded(address indexed provider, string cp);
// event ProviderRemoved(address indexed provider);
// event ProviderUpdatedWithCp(address indexed provider, string newCp);
