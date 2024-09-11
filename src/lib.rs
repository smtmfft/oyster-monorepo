mod schema;

use std::future::Future;
use std::time::Duration;

use alloy::primitives::Address;
use alloy::rpc::types::eth::Log;
use anyhow::{anyhow, Context, Result};
use diesel::connection::LoadConnection;
use diesel::prelude::*;
use tokio_stream::StreamExt;

pub trait LogsProvider {
    fn latest_block(&mut self) -> impl Future<Output = Result<i64>>;

    fn logs<'a>(
        &'a self,
        start_block: i64,
        end_block: i64,
    ) -> impl Future<Output = Result<impl StreamExt<Item = Log> + 'a>>;
}

#[derive(Clone)]
pub struct AlloyProvider {
    pub contract: Address,
}

// impl LogsProvider for AlloyProvider {
//     async fn logs<'a>(
//         &'a self,
//         client: &'a impl Provider<PubSubFrontend>,
//     ) -> Result<impl StreamExt<Item = Log> + 'a> {
//         todo!()
//     }
// }

// sqlite for testing the future
#[derive(diesel::MultiConnection)]
pub enum AnyConnection {
    Postgresql(diesel::PgConnection),
    Sqlite(diesel::SqliteConnection),
}

pub async fn event_loop(conn: &mut AnyConnection, mut provider: impl LogsProvider) -> Result<()> {
    let mut last_updated = schema::sync::table
        .select(schema::sync::block)
        .limit(1)
        .load::<i64>(conn)
        .context("failed to load last updated block")?
        .into_iter()
        .last()
        .ok_or(anyhow!(
            "no last updated block found, should never happen unless the database is corrupted"
        ))?;

    loop {
        let latest_block = provider.latest_block().await?;
        let start_block = last_updated + 1;
        let end_block = std::cmp::min(start_block + 1999, latest_block);

        let _logs = provider.logs(start_block, end_block);

        conn.transaction(|conn| {
            diesel::update(schema::sync::table)
                .set(schema::sync::block.eq(end_block))
                .execute(conn)
        })?;

        last_updated = end_block;

        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
