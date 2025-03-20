use diesel::AsExpression;
use diesel::deserialize::{self, FromSql, FromSqlRow};
use diesel::pg::sql_types::Citext;
use diesel::pg::{Pg, PgValue};
use diesel::serialize::{self, IsNull, Output, ToSql};
use diesel::sql_types::Text;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::borrow::Cow;
use std::fmt::Display;
use std::io::Write;
use std::ops::Deref;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[diesel(sql_type = Text, sql_type = Citext)]
pub struct SmallString(SmolStr);

impl SmallString {
    pub fn new(text: impl AsRef<str>) -> Self {
        Self(SmolStr::new(text))
    }
}

impl Deref for SmallString {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
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

impl Display for SmallString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ToSql<Text, Pg> for SmallString {
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        out.write_all(self.as_bytes())?;
        Ok(IsNull::No)
    }
}

impl ToSql<Citext, Pg> for SmallString {
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        out.write_all(self.as_bytes())?;
        Ok(IsNull::No)
    }
}

impl FromSql<Text, Pg> for SmallString {
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        let string = std::str::from_utf8(value.as_bytes())?;
        Ok(Self::new(string))
    }
}

impl FromSql<Citext, Pg> for SmallString {
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        let string = std::str::from_utf8(value.as_bytes())?;
        Ok(Self::new(string))
    }
}
