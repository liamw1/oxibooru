use crate::model::post::Post;
use crate::schema::{pool, pool_category, pool_name, pool_post};
use crate::string::SmallString;
use crate::time::DateTime;
use diesel::deserialize::{self, FromSql, FromSqlRow};
use diesel::dsl::sql;
use diesel::expression::{SqlLiteral, UncheckedBind};
use diesel::pg::{Pg, PgValue};
use diesel::prelude::*;
use diesel::sql_types::{Bool, Text};
use serde::Serialize;
use std::rc::Rc;

#[derive(Insertable)]
#[diesel(table_name = pool_category)]
#[diesel(check_for_backend(Pg))]
pub struct NewPoolCategory<'a> {
    pub name: &'a str,
    pub color: &'a str,
}

#[derive(AsChangeset, Identifiable, Queryable, Selectable)]
#[diesel(table_name = pool_category)]
#[diesel(check_for_backend(Pg))]
pub struct PoolCategory {
    pub id: i64,
    pub name: SmallString,
    pub color: SmallString,
    pub last_edit_time: DateTime,
}

#[derive(Debug, Clone, FromSqlRow, Serialize)]
#[diesel(sql_type = Text)]
pub struct PoolDescription(Rc<str>);

impl FromSql<Text, Pg> for PoolDescription {
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        let string = std::str::from_utf8(value.as_bytes())?;
        Ok(Self(Rc::from(string)))
    }
}

#[derive(Insertable)]
#[diesel(table_name = pool)]
#[diesel(check_for_backend(Pg))]
pub struct NewPool<'a> {
    pub category_id: i64,
    pub description: &'a str,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(PoolCategory, foreign_key = category_id))]
#[diesel(table_name = pool)]
#[diesel(check_for_backend(Pg))]
pub struct Pool {
    pub id: i64,
    pub category_id: i64,
    pub description: String,
    pub creation_time: DateTime,
    pub last_edit_time: DateTime,
}

#[derive(Insertable)]
#[diesel(table_name = pool_name)]
#[diesel(check_for_backend(Pg))]
pub struct NewPoolName<'a> {
    pub pool_id: i64,
    pub order: i32,
    pub name: &'a str,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(Pool))]
#[diesel(table_name = pool_name)]
#[diesel(primary_key(pool_id, order))]
#[diesel(check_for_backend(Pg))]
pub struct PoolName {
    pub pool_id: i64,
    pub order: i32,
    pub name: SmallString,
}

impl PoolName {
    /// Creates an expression that filters pool_name rows to only the primary names.
    /// This exists because certain queries are much slower when using the nearly-equivalent
    /// pool_name::order.eq(0) (as much as 15x). Presumably this is because Diesel uses bind
    /// parameters and the Postgres can't optimize these queries as well when it doesn't know
    /// the exact value of `order`.
    pub fn primary() -> SqlLiteral<Bool, UncheckedBind<SqlLiteral<Bool>, pool_name::order>> {
        sql("").bind(pool_name::order).sql(" = 0")
    }
}

#[derive(Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Pool), belongs_to(Post))]
#[diesel(table_name = pool_post)]
#[diesel(primary_key(pool_id, post_id))]
#[diesel(check_for_backend(Pg))]
pub struct PoolPost {
    pub pool_id: i64,
    pub post_id: i64,
    pub order: i64,
}
