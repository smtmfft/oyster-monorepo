use crate::schema::jobs;
use alloy::primitives::Address;
use alloy::rpc::types::Log;
use alloy::sol_types::SolValue;
use anyhow::Context;
use anyhow::Result;
use diesel::query_dsl::methods::FilterDsl;
use diesel::ExpressionMethods;
use diesel::PgConnection;
use diesel::RunQueryDsl;
use tracing::warn;
use tracing::{info, instrument};

#[instrument(level = "info", skip_all, parent = None, fields(block = log.block_number, idx = log.log_index))]
pub fn handle_job_opened(conn: &mut PgConnection, log: Log) -> Result<()> {
    info!(?log, "processing");

    todo!();
}

#[cfg(test)]
mod tests {
    use alloy::{primitives::LogData, rpc::types::Log};
    use anyhow::Result;
    use diesel::QueryDsl;
    use ethp::{event, keccak256};

    use crate::handlers::handle_log;
    use crate::handlers::test_db::TestDb;
    use crate::schema::providers;

    use super::*;

    #[test]
    fn test_create_new_job() -> Result<()> {
        // setup
        let mut db = TestDb::new();
        let conn = &mut db.conn;

        let contract = "0x1111111111111111111111111111111111111111".parse()?;

        diesel::insert_into(providers::table)
            .values((
                providers::id.eq("0xaAaAaAaaAaAaAaaAaAAAAAAAAaaaAaAaAaaAaaAa"),
                providers::cp.eq("some cp"),
                providers::is_active.eq(true),
            ))
            .execute(conn)?;

        assert_eq!(providers::table.count().get_result(conn), Ok(1));
        assert_eq!(
            providers::table.select(providers::all_columns).first(conn),
            Ok((
                "0xaAaAaAaaAaAaAaaAaAAAAAAAAaaaAaAaAaaAaaAa".to_owned(),
                "some cp".to_owned(),
                true
            ))
        );

        assert_eq!(jobs::table.count().get_result(conn), Ok(0));

        // log under test
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        // we do this after the timestamp to truncate beyond seconds
        let now = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(timestamp);
        let log = Log {
            block_hash: Some(keccak256!("some block").into()),
            block_number: Some(42),
            block_timestamp: None,
            log_index: Some(69),
            transaction_hash: Some(keccak256!("some tx").into()),
            transaction_index: Some(420),
            removed: false,
            inner: alloy::primitives::Log {
                address: contract,
                data: LogData::new(
                    vec![
                        event!("JobOpened(bytes32,string,address,address,uint256,uint256,uint256)")
                            .into(),
                        "0x3333333333333333333333333333333333333333333333333333333333333333"
                            .parse()?,
                        "0xaAaAaAaaAaAaAaaAaAAAAAAAAaaaAaAaAaaAaaAa"
                            .parse::<Address>()?
                            .into_word(),
                        "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                            .parse::<Address>()?
                            .into_word(),
                    ],
                    ("some metadata", 1, 2, timestamp)
                        .abi_encode_sequence()
                        .into(),
                )
                .unwrap(),
            },
        };

        // use handle_log instead of concrete handler to test dispatch
        handle_log(conn, log)?;

        // checks
        assert_eq!(jobs::table.count().get_result(conn), Ok(1));
        assert_eq!(
            jobs::table.select(jobs::all_columns).first(conn),
            Ok((
                "0x3333333333333333333333333333333333333333333333333333333333333333".to_owned(),
                "some metadata".to_owned(),
                "0xaAaAaAaaAaAaAaaAaAAAAAAAAaaaAaAaAaaAaaAa".to_owned(),
                "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
                1,
                2,
                now,
                now,
            ))
        );

        Ok(())
    }
}
