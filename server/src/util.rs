use crate::model::TableName;
use diesel::associations::HasTable;
use diesel::backend::Backend;
use diesel::deserialize::{self, FromSql, FromSqlRow};
use diesel::expression::AsExpression;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::query_builder::{AsQuery, IntoUpdateTarget, QueryFragment, QueryId, UpdateStatement};
use diesel::result::{DatabaseErrorKind, Error};
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::Timestamptz;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use time::error::ComponentRange;
use time::serde::rfc3339;
use time::{Date, Month, OffsetDateTime, PrimitiveDateTime};

pub struct Timer<'a> {
    name: &'a str,
    start: std::time::Instant,
}

impl<'a> Timer<'a> {
    pub fn new(name: &'a str) -> Self {
        Self {
            name,
            start: std::time::Instant::now(),
        }
    }
}

impl<'a> Drop for Timer<'a> {
    fn drop(&mut self) {
        let elapsed_time = self.start.elapsed();
        println!("{} took {}ms", self.name, elapsed_time.as_millis());
    }
}

// A wrapper for time::OffsetDateTime that serializes/deserializes according to RFC 3339.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[diesel(sql_type = Timestamptz)]
pub struct DateTime(#[serde(with = "rfc3339")] OffsetDateTime);

impl DateTime {
    pub fn now() -> Self {
        OffsetDateTime::now_utc().into()
    }

    pub fn today() -> Self {
        Self::now().date().midnight().assume_utc().into()
    }

    pub fn yesterday() -> Self {
        Self::now()
            .date()
            .previous_day()
            .unwrap()
            .midnight()
            .assume_utc()
            .into()
    }

    pub fn from_date(year: i32, month: u8, day: u8) -> Result<Self, ComponentRange> {
        Month::try_from(month)
            .and_then(|month| Date::from_calendar_date(year, month, day))
            .map(Date::midnight)
            .map(PrimitiveDateTime::assume_utc)
            .map(Self::from)
    }
}

impl Deref for DateTime {
    type Target = OffsetDateTime;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for DateTime {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<OffsetDateTime> for DateTime {
    fn from(value: OffsetDateTime) -> Self {
        DateTime(value)
    }
}

impl<DB: Backend> ToSql<Timestamptz, DB> for DateTime
where
    OffsetDateTime: ToSql<diesel::sql_types::Timestamptz, DB>,
{
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, DB>) -> serialize::Result {
        self.0.to_sql(out)
    }
}

impl<DB: Backend> FromSql<Timestamptz, DB> for DateTime
where
    OffsetDateTime: FromSql<diesel::sql_types::Timestamptz, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> deserialize::Result<Self> {
        OffsetDateTime::from_sql(bytes).map(DateTime)
    }
}

pub fn delete<R>(conn: &mut PgConnection, row: R) -> QueryResult<()>
where
    R: Deref + IntoUpdateTarget,
    <<R as HasTable>::Table as QuerySource>::FromClause: QueryFragment<Pg>,
    <R as IntoUpdateTarget>::WhereClause: QueryFragment<Pg>,
    <R as IntoUpdateTarget>::WhereClause: QueryId,
    <R as HasTable>::Table: QueryId,
    <R as HasTable>::Table: 'static,
    R::Target: TableName,
{
    conn.transaction(|conn| validate_uniqueness(R::Target::table_name(), "delete", diesel::delete(row).execute(conn)?))
}

pub fn update_single_row<R, T, V>(conn: &mut PgConnection, row: R, values: V) -> QueryResult<()>
where
    R: Deref + IntoUpdateTarget<Table = T>,
    <<R as HasTable>::Table as QuerySource>::FromClause: QueryFragment<Pg>,
    <R as IntoUpdateTarget>::WhereClause: QueryFragment<Pg>,
    V: diesel::AsChangeset<Target = T>,
    T: diesel::QuerySource + diesel::Table,
    UpdateStatement<T, <R as IntoUpdateTarget>::WhereClause, <V as AsChangeset>::Changeset>: AsQuery,
    <V as diesel::AsChangeset>::Changeset: QueryFragment<Pg>,
    R::Target: TableName,
{
    conn.transaction(|conn| {
        validate_uniqueness(R::Target::table_name(), "update", diesel::update(row).set(values).execute(conn)?)
    })
}

fn validate_uniqueness(table_name: &str, transaction_type: &str, rows_changed: usize) -> QueryResult<()> {
    match rows_changed {
        0 => Err(Error::NotFound),
        1 => Ok(()),
        _ => Err(Error::DatabaseError(
            DatabaseErrorKind::UniqueViolation,
            Box::new(format!("Failed to {transaction_type} {table_name}: entry is not unique")),
        )),
    }
}
