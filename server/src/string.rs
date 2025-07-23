use compact_str::CompactString;
use diesel::AsExpression;
use diesel::deserialize::{self, FromSql, FromSqlRow};
use diesel::pg::sql_types::Citext;
use diesel::pg::{Pg, PgValue};
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::Text;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt::Display;
use std::ops::Deref;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[diesel(sql_type = Text, sql_type = Citext)]
pub struct SmallString(CompactString);

impl SmallString {
    pub fn new(text: impl AsRef<str>) -> Self {
        Self(CompactString::new(text))
    }

    pub fn to_lowercase(&self) -> Self {
        Self(self.0.to_lowercase())
    }
}

impl Deref for SmallString {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for SmallString {
    type Err = core::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        CompactString::from_str(s).map(Self)
    }
}

impl From<String> for SmallString {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<Cow<'_, str>> for SmallString {
    fn from(value: Cow<str>) -> Self {
        Self::new(value)
    }
}

impl From<i64> for SmallString {
    fn from(value: i64) -> Self {
        value.to_string().into()
    }
}

impl Display for SmallString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl ToSql<Text, Pg> for SmallString {
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Pg>) -> serialize::Result {
        <str as ToSql<Text, Pg>>::to_sql(self.0.as_str(), out)
    }
}

impl ToSql<Citext, Pg> for SmallString {
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Pg>) -> serialize::Result {
        <str as ToSql<Citext, Pg>>::to_sql(self.0.as_str(), out)
    }
}

impl<T> FromSql<T, Pg> for SmallString
where
    String: deserialize::FromSql<T, Pg>,
{
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        CompactString::from_utf8(value.as_bytes()).map(Self).map_err(Box::from)
    }
}
