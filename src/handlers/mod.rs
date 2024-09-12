use alloy::rpc::types::Log;
use anyhow::anyhow;
use anyhow::Result;
use diesel::PgConnection;
use ethp::event;
use tracing::warn;
use tracing::{info, instrument};

mod provider_added;
use provider_added::handle_provider_added;

mod provider_removed;
use provider_removed::handle_provider_removed;

mod provider_updated_with_cp;
use provider_updated_with_cp::handle_provider_updated_with_cp;

// provider logs
static PROVIDER_ADDED_TOPIC: [u8; 32] = event!("ProviderAdded(address,string)");
static PROVIDER_REMOVED_TOPIC: [u8; 32] = event!("ProviderRemoved(address)");
static PROVIDER_UPDATED_WITH_CP_TOPIC: [u8; 32] = event!("ProviderUpdatedWithCp(address,string)");

// job logs
static JOB_OPENED_TOPIC: [u8; 32] =
    event!("JobOpened(bytes32,string,address,address,uint256,uint256,uint256)");
static JOB_SETTLED_TOPIC: [u8; 32] = event!("JobSettled(bytes32,uint256,uint256)");
static JOB_CLOSED_TOPIC: [u8; 32] = event!("JobClosed(bytes32)");
static JOB_DEPOSITED_TOPIC: [u8; 32] = event!("JobDeposited(bytes32,address,uint256)");
static JOB_WITHDREW_TOPIC: [u8; 32] = event!("JobWithdrew(bytes32,address,uint256)");
static JOB_REVISE_RATE_INITIATED_TOPIC: [u8; 32] =
    event!("JobReviseRateInitiated(bytes32,uint256)");
static JOB_REVISE_RATE_CANCELLED_TOPIC: [u8; 32] = event!("JobReviseRateCancelled(bytes32)");
static JOB_REVISE_RATE_FINALIZED_TOPIC: [u8; 32] =
    event!("JobReviseRateFinalized(bytes32,uint256)");
static JOB_METADATA_UPDATED_TOPIC: [u8; 32] = event!("JobMetadataUpdated(bytes32,string)");

// ignored logs
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
pub fn handle_log(conn: &mut PgConnection, log: Log) -> Result<()> {
    info!(?log, "processing");

    let log_type = log
        .topic0()
        .ok_or(anyhow!("log does not have topic0, should never happen"))?;

    if log_type == PROVIDER_ADDED_TOPIC {
        handle_provider_added(conn, log)
    } else if log_type == PROVIDER_REMOVED_TOPIC {
        handle_provider_removed(conn, log)
    } else if log_type == PROVIDER_UPDATED_WITH_CP_TOPIC {
        handle_provider_updated_with_cp(conn, log)
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

#[cfg(test)]
mod test_db;
