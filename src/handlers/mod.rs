use crate::schema::providers;
use alloy::primitives::Address;
use alloy::rpc::types::Log;
use alloy::sol_types::SolValue;
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use diesel::ExpressionMethods;
use diesel::PgConnection;
use diesel::RunQueryDsl;
use ethp::event;
use tracing::warn;
use tracing::{info, instrument};

mod provider_added;
use provider_added::handle_provider_added;

mod provider_removed;
use provider_removed::handle_provider_removed;

// provider logs
static PROVIDER_ADDED_TOPIC: [u8; 32] = event!("ProviderAdded(address,string)");
static PROVIDER_REMOVED_TOPIC: [u8; 32] = event!("ProviderRemoved(address)");
static PROVIDER_UPDATED_WITH_CP_TOPIC: [u8; 32] = event!("ProviderUpdatedWithCp(address,string)");

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

#[instrument(level = "info", skip_all, parent = None, fields(block = log.block_number, idx = log.log_index))]
pub fn handle_provider_updated_with_cp(conn: &mut PgConnection, log: Log) -> Result<()> {
    info!(?log, "processing");

    let provider = Address::from_word(log.topics()[1]).to_checksum(None);
    let cp = String::abi_decode(&log.data().data, true)?;

    info!(provider, "updating provider");
    let count = diesel::update(providers::table)
        .filter(providers::id.eq(&provider))
        .set(providers::cp.eq(cp))
        .execute(conn)
        .context("failed to update provider")?;

    // warn just in case
    if count != 1 {
        warn!(count, "count should have been 1");
    }

    info!(provider, "updated provider");

    Ok(())
}

#[cfg(test)]
mod test_db;
