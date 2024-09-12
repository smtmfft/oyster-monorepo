use std::env;

use anyhow::Result;
use diesel::Connection;
use diesel::PgConnection;
use dotenvy::dotenv;

use oyster_indexer::event_loop;
use oyster_indexer::start_from;
use oyster_indexer::AlloyProvider;

fn main() -> Result<()> {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let conn = PgConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url));

    let mut conn = oyster_indexer::AnyConnection::Postgresql(conn);
    let provider = AlloyProvider {
        url: "https://arb1.arbitrum.io/rpc".parse()?,
        contract: "0x9d95D61eA056721E358BC49fE995caBF3B86A34B".parse()?,
    };
    let is_start_set = start_from(&mut conn, 87252070)?;
    println!("is_start_set: {}", is_start_set);
    event_loop(&mut conn, provider)
}
