use diesel::deserialize::{self, FromSql, FromSqlRow};
use diesel::expression::AsExpression;
use diesel::pg::{Pg, PgValue};
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::Timestamptz;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use time::error::ComponentRange;
use time::serde::rfc3339;
use time::{Date, Month, OffsetDateTime, PrimitiveDateTime};
use tracing::info;

/// Used for timing things. Prints how long the object lived when dropped.
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

impl Drop for Timer<'_> {
    fn drop(&mut self) {
        let elapsed_time = self.start.elapsed();
        let time_in_s = elapsed_time.as_secs_f64();
        match elapsed_time.as_nanos().ilog10() {
            0..3 => info!("{} took {:.1}ns", self.name, time_in_s * 1e9),
            3..6 => info!("{} took {:.3}μs", self.name, time_in_s * 1e6),
            6..9 => info!("{} took {:.3}ms", self.name, time_in_s * 1e3),
            9..12 => info!("{} took {:.3}s", self.name, time_in_s),
            12.. => info!("{} took {:.0}s", self.name, time_in_s),
        }
    }
}

/// A wrapper for [`OffsetDateTime`] that serializes/deserializes according to RFC 3339.
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
            .unwrap_or(Date::MIN)
            .midnight()
            .assume_utc()
            .into()
    }

    pub fn tomorrow() -> Self {
        Self::now()
            .date()
            .next_day()
            .unwrap_or(Date::MAX)
            .midnight()
            .assume_utc()
            .into()
    }

    pub fn from_date(year: i32, month: Month, day: u8) -> Result<Self, ComponentRange> {
        Date::from_calendar_date(year, month, day)
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
        Self(value)
    }
}

impl ToSql<Timestamptz, Pg> for DateTime {
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Pg>) -> serialize::Result {
        <OffsetDateTime as ToSql<Timestamptz, Pg>>::to_sql(&self.0, out)
    }
}

impl FromSql<Timestamptz, Pg> for DateTime {
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        OffsetDateTime::from_sql(value).map(DateTime)
    }
}
