use std::env;

use alloy::rpc::types::eth::Log;
use anyhow::Result;
use diesel::Connection;
use diesel::PgConnection;
use dotenvy::dotenv;

use oyster_indexer::event_loop;
use oyster_indexer::LogsProvider;

struct DummyProvider {
    x: i64,
}

impl LogsProvider for DummyProvider {
    fn latest_block(&mut self) -> Result<i64> {
        self.x += 10;
        Ok(self.x)
    }

    fn logs(
        &self,
        _start_block: i64,
        _end_block: i64,
    ) -> anyhow::Result<impl IntoIterator<Item = Log>> {
        Ok([].into_iter())
    }
}

fn main() -> Result<()> {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let conn = PgConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url));

    let provider = DummyProvider { x: 0 };
    event_loop(
        &mut oyster_indexer::AnyConnection::Postgresql(conn),
        provider,
    )
}
