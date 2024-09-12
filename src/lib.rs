mod schema;

use std::time::Duration;

use alloy::primitives::Address;
use alloy::providers::Provider;
use alloy::rpc::types::eth::Log;
use alloy::rpc::types::Filter;
use alloy::transports::http::reqwest::Url;
use anyhow::{anyhow, Context, Result};
use diesel::connection::LoadConnection;
use diesel::prelude::*;

pub trait LogsProvider {
    fn latest_block(&mut self) -> Result<u64>;
    fn logs(&self, start_block: u64, end_block: u64) -> Result<impl IntoIterator<Item = Log>>;
}

#[derive(Clone)]
pub struct AlloyProvider {
    pub url: Url,
    pub contract: Address,
}

impl LogsProvider for AlloyProvider {
    fn latest_block(&mut self) -> Result<u64> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        Ok(rt.block_on(
            alloy::providers::ProviderBuilder::new()
                .on_http(self.url.clone())
                .get_block_number(),
        )?)
    }

    fn logs(&self, start_block: u64, end_block: u64) -> Result<impl IntoIterator<Item = Log>> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        Ok(rt.block_on(
            alloy::providers::ProviderBuilder::new()
                .on_http(self.url.clone())
                .get_logs(
                    &Filter::new()
                        .from_block(start_block)
                        .to_block(end_block)
                        .address(self.contract),
                ),
        )?)
    }
}

// sqlite for testing the future
#[derive(diesel::MultiConnection)]
pub enum AnyConnection {
    Postgresql(diesel::PgConnection),
    Sqlite(diesel::SqliteConnection),
}

pub fn event_loop(conn: &mut AnyConnection, mut provider: impl LogsProvider) -> Result<()> {
    // fetch last updated block from the db
    let mut last_updated = schema::sync::table
        .select(schema::sync::block)
        .limit(1)
        .load::<i64>(conn)
        .context("failed to fetch last updated block")?
        .into_iter()
        .last()
        .ok_or(anyhow!(
            "no last updated block found, should never happen unless the database is corrupted"
        ))? as u64;

    loop {
        // fetch latest block from the rpc
        let latest_block = provider.latest_block()?;

        // should not really ever be true
        // effectively means the rpc was rolled back
        if latest_block < last_updated {
            return Err(anyhow!(
                "rpc is behind the db, should never happen unless the rpc was rolled back"
            ));
        }

        if latest_block == last_updated {
            // we are up to date, simply sleep for a bit
            std::thread::sleep(Duration::from_secs(5));
            continue;
        }

        // start from the next block to what has already been processed
        let start_block = last_updated + 1;
        // cap block range to 2000, seems to be a popular rate limit
        let end_block = std::cmp::min(start_block + 1999, latest_block);

        let logs = provider.logs(start_block, end_block)?;
        println!("{:?}", logs.into_iter().collect::<Vec<Log>>());

        // execute db writes within a transaction for consistency
        // NOTE: diesel transactions are synchronous, async is not allowed inside
        // might be limiting for certain things like making rpc queries while processing events
        // using a temporary tokio runtime is a possibility
        conn.transaction(|conn| {
            diesel::update(schema::sync::table)
                .set(schema::sync::block.eq(end_block as i64))
                .execute(conn)
        })?;

        last_updated = end_block;

        std::thread::sleep(Duration::from_secs(2));
    }
}
