use std::ops::Sub;
use std::str::FromStr;

use crate::schema::jobs;
use crate::schema::revise_rate_requests;
use alloy::hex::ToHexExt;
use alloy::primitives::U256;
use alloy::rpc::types::Log;
use alloy::sol_types::SolValue;
use anyhow::Context;
use anyhow::Result;
use bigdecimal::BigDecimal;
use diesel::ExpressionMethods;
use diesel::PgConnection;
use diesel::RunQueryDsl;
use tracing::warn;
use tracing::{info, instrument};

use super::status::Status;

#[instrument(level = "info", skip_all, parent = None, fields(block = log.block_number, idx = log.log_index))]
pub fn handle_job_revise_rate_cancelled(conn: &mut PgConnection, log: Log) -> Result<()> {
    info!(?log, "processing");

    let id = log.topics()[1].encode_hex_with_prefix();

    info!(id, "cancelling revise rate request");

    diesel::update(revise_rate_requests::table)
        .filter(revise_rate_requests::id.eq(&id))
        .set(revise_rate_requests::status.eq(Status::Cancelled))
        .execute(conn)
        .context("failed to update job")?;

    info!(id, "cancelled revise rate request");

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::ops::Add;
    use std::time::Duration;

    use alloy::primitives::Bytes;
    use alloy::{primitives::LogData, rpc::types::Log};
    use anyhow::Result;
    use bigdecimal::BigDecimal;
    use diesel::QueryDsl;
    use ethp::{event, keccak256};

    use crate::handlers::handle_log;
    use crate::handlers::test_db::TestDb;
    use crate::schema::{jobs, providers, revise_rate_requests};

    use super::*;

    #[test]
    fn test_revise_rate_cancel() -> Result<()> {
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

        let original_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        // we do this after the timestamp to truncate beyond seconds
        let original_now =
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(original_timestamp);
        diesel::insert_into(jobs::table)
            .values((
                jobs::id.eq("0x4444444444444444444444444444444444444444444444444444444444444444"),
                jobs::owner.eq("0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB"),
                jobs::provider.eq("0xaAaAaAaaAaAaAaaAaAAAAAAAAaaaAaAaAaaAaaAa"),
                jobs::metadata.eq("some other metadata"),
                jobs::rate.eq(BigDecimal::from(3)),
                jobs::balance.eq(BigDecimal::from(21)),
                jobs::last_settled.eq(&original_now),
                jobs::created.eq(&original_now),
                jobs::is_closed.eq(false),
            ))
            .execute(conn)
            .context("failed to create job")?;
        let creation_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        // we do this after the timestamp to truncate beyond seconds
        let creation_now =
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(creation_timestamp);
        diesel::insert_into(jobs::table)
            .values((
                jobs::id.eq("0x3333333333333333333333333333333333333333333333333333333333333333"),
                jobs::owner.eq("0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB"),
                jobs::provider.eq("0xaAaAaAaaAaAaAaaAaAAAAAAAAaaaAaAaAaaAaaAa"),
                jobs::metadata.eq("some metadata"),
                jobs::rate.eq(BigDecimal::from(1)),
                jobs::balance.eq(BigDecimal::from(20)),
                jobs::last_settled.eq(&creation_now),
                jobs::created.eq(&creation_now),
                jobs::is_closed.eq(false),
            ))
            .execute(conn)
            .context("failed to create job")?;
        let revise_now = original_now.add(Duration::from_secs(300));
        diesel::insert_into(revise_rate_requests::table)
            .values((
                revise_rate_requests::id
                    .eq("0x4444444444444444444444444444444444444444444444444444444444444444"),
                revise_rate_requests::value.eq(BigDecimal::from(2)),
                revise_rate_requests::updates_at.eq(&revise_now),
                revise_rate_requests::status.eq(Status::Completed),
            ))
            .execute(conn)
            .context("failed to create revise rate request")?;
        diesel::insert_into(revise_rate_requests::table)
            .values((
                revise_rate_requests::id
                    .eq("0x3333333333333333333333333333333333333333333333333333333333333333"),
                revise_rate_requests::value.eq(BigDecimal::from(5)),
                revise_rate_requests::updates_at.eq(&creation_now.add(Duration::from_secs(600))),
                revise_rate_requests::status.eq(Status::InProgress),
            ))
            .execute(conn)
            .context("failed to create revise rate request")?;

        assert_eq!(providers::table.count().get_result(conn), Ok(1));
        assert_eq!(
            providers::table.select(providers::all_columns).first(conn),
            Ok((
                "0xaAaAaAaaAaAaAaaAaAAAAAAAAaaaAaAaAaaAaaAa".to_owned(),
                "some cp".to_owned(),
                true
            ))
        );

        assert_eq!(jobs::table.count().get_result(conn), Ok(2));
        assert_eq!(
            jobs::table
                .select(jobs::all_columns)
                .order_by(jobs::id)
                .load(conn),
            Ok(vec![
                (
                    "0x3333333333333333333333333333333333333333333333333333333333333333".to_owned(),
                    "some metadata".to_owned(),
                    "0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB".to_owned(),
                    "0xaAaAaAaaAaAaAaaAaAAAAAAAAaaaAaAaAaaAaaAa".to_owned(),
                    BigDecimal::from(1),
                    BigDecimal::from(20),
                    creation_now,
                    creation_now,
                    false,
                ),
                (
                    "0x4444444444444444444444444444444444444444444444444444444444444444".to_owned(),
                    "some other metadata".to_owned(),
                    "0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB".to_owned(),
                    "0xaAaAaAaaAaAaAaaAaAAAAAAAAaaaAaAaAaaAaaAa".to_owned(),
                    BigDecimal::from(3),
                    BigDecimal::from(21),
                    original_now,
                    original_now,
                    false,
                )
            ])
        );

        assert_eq!(revise_rate_requests::table.count().get_result(conn), Ok(2));
        assert_eq!(
            revise_rate_requests::table
                .select(revise_rate_requests::all_columns)
                .order_by(revise_rate_requests::id)
                .load(conn),
            Ok(vec![
                (
                    "0x3333333333333333333333333333333333333333333333333333333333333333".to_owned(),
                    BigDecimal::from(5),
                    creation_now.add(Duration::from_secs(600)),
                    Status::InProgress,
                ),
                (
                    "0x4444444444444444444444444444444444444444444444444444444444444444".to_owned(),
                    BigDecimal::from(2),
                    revise_now,
                    Status::Completed,
                )
            ])
        );

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
                        event!("JobReviseRateCancelled(bytes32)").into(),
                        "0x3333333333333333333333333333333333333333333333333333333333333333"
                            .parse()?,
                    ],
                    Bytes::new(),
                )
                .unwrap(),
            },
        };

        // use handle_log instead of concrete handler to test dispatch
        handle_log(conn, log)?;

        // checks
        assert_eq!(providers::table.count().get_result(conn), Ok(1));
        assert_eq!(
            providers::table.select(providers::all_columns).first(conn),
            Ok((
                "0xaAaAaAaaAaAaAaaAaAAAAAAAAaaaAaAaAaaAaaAa".to_owned(),
                "some cp".to_owned(),
                true
            ))
        );

        assert_eq!(jobs::table.count().get_result(conn), Ok(2));
        assert_eq!(
            jobs::table
                .select(jobs::all_columns)
                .order_by(jobs::id)
                .load(conn),
            Ok(vec![
                (
                    "0x3333333333333333333333333333333333333333333333333333333333333333".to_owned(),
                    "some metadata".to_owned(),
                    "0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB".to_owned(),
                    "0xaAaAaAaaAaAaAaaAaAAAAAAAAaaaAaAaAaaAaaAa".to_owned(),
                    BigDecimal::from(1),
                    BigDecimal::from(20),
                    creation_now,
                    creation_now,
                    false,
                ),
                (
                    "0x4444444444444444444444444444444444444444444444444444444444444444".to_owned(),
                    "some other metadata".to_owned(),
                    "0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB".to_owned(),
                    "0xaAaAaAaaAaAaAaaAaAAAAAAAAaaaAaAaAaaAaaAa".to_owned(),
                    BigDecimal::from(3),
                    BigDecimal::from(21),
                    original_now,
                    original_now,
                    false,
                )
            ])
        );

        assert_eq!(revise_rate_requests::table.count().get_result(conn), Ok(2));
        assert_eq!(
            revise_rate_requests::table
                .select(revise_rate_requests::all_columns)
                .order_by(revise_rate_requests::id)
                .load(conn),
            Ok(vec![
                (
                    "0x3333333333333333333333333333333333333333333333333333333333333333".to_owned(),
                    BigDecimal::from(5),
                    creation_now.add(Duration::from_secs(600)),
                    Status::Cancelled,
                ),
                (
                    "0x4444444444444444444444444444444444444444444444444444444444444444".to_owned(),
                    BigDecimal::from(2),
                    revise_now,
                    Status::Completed,
                )
            ])
        );

        Ok(())
    }
}
