use std::io::Write;

use crate::schema::sql_types::RequestStatus;
use diesel::deserialize::FromSql;
use diesel::deserialize::FromSqlRow;
use diesel::expression::AsExpression;
use diesel::pg::Pg;
use diesel::pg::PgValue;
use diesel::serialize::IsNull;
use diesel::serialize::Output;
use diesel::serialize::ToSql;

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
