use crate::schema::providers;
use alloy::primitives::Address;
use alloy::rpc::types::Log;
use alloy::sol_types::SolValue;
use anyhow::Context;
use anyhow::Result;
use diesel::ExpressionMethods;
use diesel::PgConnection;
use diesel::RunQueryDsl;
use tracing::warn;
use tracing::{info, instrument};

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

    if count != 1 {
        // !!! should never happen
        // we should have had exactly one row made inactive
        // if count is 0, that means the provider did not exist or was not active
        // if count is more than 1, there was somehow more than one provider entry
        // we error out for now, can consider just moving on
        return Err(anyhow::anyhow!("count {count} should have been 1"));
    }

    info!(provider, "updated provider");

    Ok(())
}
