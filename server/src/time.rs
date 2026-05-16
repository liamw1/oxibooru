use crate::unit;
use diesel::deserialize::{self, FromSql, FromSqlRow};
use diesel::expression::AsExpression;
use diesel::pg::{Pg, PgValue};
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::Timestamptz;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::ops::{Deref, DerefMut, Sub};
use time::error::ComponentRange;
use time::serde::rfc3339;
use time::{Date, Duration, Month, OffsetDateTime, PrimitiveDateTime};
use tracing::info;
use utoipa::ToSchema;

pub const BUILD_DATE: DateTime = build_date();

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
        let elasped_time = unit::format_duration(self.start.elapsed());
        info!("{} took {elasped_time}", self.name);
    }
}

/// A wrapper for [`OffsetDateTime`] that serializes/deserializes according to RFC 3339.
#[allow(clippy::unsafe_derive_deserialize)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, AsExpression, FromSqlRow, ToSchema,
)]
#[diesel(sql_type = Timestamptz)]
#[schema(description = "A RFC 3339 formatted datetime string")]
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
        let date = Date::from_calendar_date(year, month, day)?;
        let date_time = Date::midnight(date);
        let utc_time = PrimitiveDateTime::assume_utc(date_time);
        Ok(Self(utc_time))
    }

    #[cfg(test)]
    pub const fn test_date() -> Self {
        Self(time::macros::datetime!(2008-09-15 0:00 UTC))
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

impl Sub for DateTime {
    type Output = Duration;
    fn sub(self, rhs: Self) -> Self::Output {
        self.0 - rhs.0
    }
}

impl Display for DateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<OffsetDateTime> for DateTime {
    fn from(value: OffsetDateTime) -> Self {
        Self(value)
    }
}

impl ToSql<Timestamptz, Pg> for DateTime {
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Pg>) -> serialize::Result {
        <OffsetDateTime as ToSql<Timestamptz, Pg>>::to_sql(self, out)
    }
}

impl FromSql<Timestamptz, Pg> for DateTime {
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        OffsetDateTime::from_sql(value).map(DateTime)
    }
}

pub fn since(time: DateTime) -> String {
    let duration_since_build = DateTime::now() - time;
    let seconds = duration_since_build.whole_seconds();
    let minutes = duration_since_build.whole_minutes();
    let hours = duration_since_build.whole_hours();
    let days = duration_since_build.whole_days();
    let weeks = duration_since_build.whole_weeks();
    let months = weeks / 4;
    let years = weeks / 52;

    if years > 1 {
        format!("{years} years ago")
    } else if years == 1 {
        String::from("one year ago")
    } else if months > 1 {
        format!("{months} months ago")
    } else if months == 1 {
        String::from("one month ago")
    } else if days > 1 {
        format!("{days} days ago")
    } else if days == 1 {
        String::from("one day ago")
    } else if hours > 1 {
        format!("{hours} hours ago")
    } else if hours == 1 {
        String::from("one hour ago")
    } else if minutes > 1 {
        format!("{minutes} minutes ago")
    } else if minutes == 1 {
        String::from("one minute ago")
    } else if seconds > 1 {
        format!("{seconds} seconds ago")
    } else {
        format!("just now")
    }
}

// Import stash build timestamp
include!(concat!(env!("OUT_DIR"), "/build_timestamp.rs"));

const fn build_date() -> DateTime {
    let time = match OffsetDateTime::from_unix_timestamp(BUILD_TIMESTAMP) {
        Ok(time) => time,
        Err(_) => panic!("Invalid build timestamp"),
    };
    DateTime(time)
}
