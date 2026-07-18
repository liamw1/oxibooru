use compact_str::{CompactString, ToCompactString};
use diesel::AsExpression;
use diesel::deserialize::{self, FromSql, FromSqlRow};
use diesel::pg::sql_types::Citext;
use diesel::pg::{Pg, PgValue};
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::Text;
use serde::{Deserialize, Deserializer, Serialize};
use std::borrow::Cow;
use std::convert::Infallible;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;
use utoipa::ToSchema;

/// A wrapper over a [`String`] that's meant to contain sensitive data.
/// Uses custom [`Debug`] implementation so that text can't be accidentally leaked
/// via panic logs.
#[derive(PartialEq, Eq, Deserialize, AsExpression, FromSqlRow, ToSchema)]
#[diesel(sql_type = Text, sql_type = Citext)]
#[schema(value_type = String, description = "")]
pub struct SecretString(String);

impl SecretString {
    pub const fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    pub const fn read(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl From<&str> for SecretString {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for SecretString {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl FromIterator<char> for SecretString {
    fn from_iter<T: IntoIterator<Item = char>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "SecretString([REDACTED])")
    }
}

impl ToSql<Text, Pg> for SecretString {
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Pg>) -> serialize::Result {
        <str as ToSql<Text, Pg>>::to_sql(&self.0, out)
    }
}

impl ToSql<Citext, Pg> for SecretString {
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Pg>) -> serialize::Result {
        <str as ToSql<Citext, Pg>>::to_sql(&self.0, out)
    }
}

impl FromSql<Text, Pg> for SecretString {
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        str::from_utf8(value.as_bytes()).map(Self::from).map_err(Box::from)
    }
}

impl FromSql<Citext, Pg> for SecretString {
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        str::from_utf8(value.as_bytes()).map(Self::from).map_err(Box::from)
    }
}

/// A wrapper over [`CompactString`] that can be serialized to or deserialized from the database.
/// Implements Small String Optimization (SSO), so it doesn't allocate if the length is 24 bytes or less.
/// Ideal for strings that are typically short, such as tag names.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, AsExpression, FromSqlRow, ToSchema,
)]
#[diesel(sql_type = Text, sql_type = Citext)]
#[schema(value_type = String, description = "")]
pub struct SmallString(CompactString);

impl SmallString {
    pub fn new(text: impl AsRef<str>) -> Self {
        Self(CompactString::new(text))
    }
}

impl Deref for SmallString {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for SmallString {
    type Err = Infallible;
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
        Self(value.to_compact_string())
    }
}

impl Display for SmallString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl ToSql<Text, Pg> for SmallString {
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Pg>) -> serialize::Result {
        <str as ToSql<Text, Pg>>::to_sql(self, out)
    }
}

impl ToSql<Citext, Pg> for SmallString {
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Pg>) -> serialize::Result {
        <str as ToSql<Citext, Pg>>::to_sql(self, out)
    }
}

impl FromSql<Text, Pg> for SmallString {
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        CompactString::from_utf8(value.as_bytes()).map(Self).map_err(Box::from)
    }
}

impl FromSql<Citext, Pg> for SmallString {
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        CompactString::from_utf8(value.as_bytes()).map(Self).map_err(Box::from)
    }
}

/// A wrapper over [`Arc<str>`] that can be serialized to or deserialized from the database.
/// It's immutable, but can be cheaply cloned and sent across threads.
/// Meant for potentially large string, like post descriptions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, AsExpression, FromSqlRow, ToSchema)]
#[diesel(sql_type = Text)]
#[schema(value_type = String, description = "")]
pub struct LargeString(Arc<str>);

impl Deref for LargeString {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for LargeString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl FromSql<Text, Pg> for LargeString {
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        str::from_utf8(value.as_bytes())
            .map(Arc::from)
            .map(Self)
            .map_err(Box::from)
    }
}

impl ToSql<Text, Pg> for LargeString {
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Pg>) -> serialize::Result {
        <str as ToSql<Text, Pg>>::to_sql(self, out)
    }
}

impl<'de> Deserialize<'de> for LargeString {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(|s| Arc::from(s.trim())).map(Self)
    }
}
