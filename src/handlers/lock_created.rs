use std::io::Write;
use std::ops::Add;
use std::str::FromStr;

use crate::schema::jobs;
use crate::schema::sql_types::RequestStatus;
use alloy::hex::ToHexExt;
use alloy::primitives::U256;
use alloy::rpc::types::Log;
use alloy::sol_types::SolValue;
use anyhow::Context;
use anyhow::Result;
use bigdecimal::BigDecimal;
use diesel::deserialize::FromSql;
use diesel::deserialize::FromSqlRow;
use diesel::expression::AsExpression;
use diesel::pg::Pg;
use diesel::pg::PgValue;
use diesel::serialize::IsNull;
use diesel::serialize::Output;
use diesel::serialize::ToSql;
use diesel::ExpressionMethods;
use diesel::PgConnection;
use diesel::RunQueryDsl;
use ethp::keccak256;
use tracing::warn;
use tracing::{info, instrument};

static RATE_LOCK_SELECTOR: [u8; 32] = keccak256!("RATE_LOCK");

#[derive(Debug, PartialEq, FromSqlRow, AsExpression, Eq)]
#[diesel(sql_type = RequestStatus)]
pub enum Status {
    InProgress,
    Cancelled,
    Completed,
}

impl ToSql<RequestStatus, Pg> for Status {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> diesel::serialize::Result {
        match *self {
            Status::InProgress => out.write_all(b"IN_PROGRESS")?,
            Status::Cancelled => out.write_all(b"CANCELLED")?,
            Status::Completed => out.write_all(b"COMPLETED")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<RequestStatus, Pg> for Status {
    fn from_sql(bytes: PgValue<'_>) -> diesel::deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"IN_PROGRESS" => Ok(Status::InProgress),
            b"CANCELLED" => Ok(Status::Cancelled),
            b"COMPLETED" => Ok(Status::Completed),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

#[instrument(level = "info", skip_all, parent = None, fields(block = log.block_number, idx = log.log_index))]
pub fn handle_lock_created(conn: &mut PgConnection, log: Log) -> Result<()> {
    info!(?log, "processing");

    todo!()
}
