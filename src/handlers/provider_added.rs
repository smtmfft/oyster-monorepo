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
pub fn handle_provider_added(conn: &mut PgConnection, log: Log) -> Result<()> {
    info!(?log, "processing");

    let provider = Address::from_word(log.topics()[1]).to_checksum(None);
    let cp = String::abi_decode(&log.data().data, true)?;

    info!(provider, cp, "inserting provider");
    diesel::insert_into(providers::table)
        .values((
            providers::id.eq(&provider),
            providers::cp.eq(&cp),
            providers::is_active.eq(true),
        ))
        .execute(conn)
        .context("failed to add provider")?;
    info!(provider, cp, "inserted provider");

    Ok(())
}
