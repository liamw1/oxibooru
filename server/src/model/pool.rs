use crate::model::post::Post;
use crate::schema::{pool, pool_category, pool_name, pool_post};
use crate::util::DateTime;
use diesel::deserialize::{self, FromSql, FromSqlRow};
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::Integer;
use diesel::AsExpression;
use serde::Serialize;

#[derive(Insertable)]
#[diesel(table_name = pool_category)]
pub struct NewPoolCategory<'a> {
    pub name: &'a str,
    pub color: &'a str,
}

#[derive(Identifiable, Queryable, Selectable)]
#[diesel(table_name = pool_category)]
#[diesel(check_for_backend(Pg))]
pub struct PoolCategory {
    pub id: i32,
    pub name: String,
    pub color: String,
    pub last_edit_time: DateTime,
}

#[derive(Insertable)]
#[diesel(table_name = pool)]
pub struct NewPool {
    pub category_id: i32,
}

#[derive(Debug, Clone, Copy, Associations, Selectable, AsExpression, FromSqlRow)]
#[diesel(sql_type = Integer)]
#[diesel(belongs_to(Pool, foreign_key = id))]
#[diesel(table_name = pool)]
#[diesel(check_for_backend(Pg))]
pub struct PoolId {
    pub id: i32,
}

impl ToSql<Integer, Pg> for PoolId
where
    i32: ToSql<Integer, Pg>,
{
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        <i32 as ToSql<Integer, Pg>>::to_sql(&self.id, &mut out.reborrow())
    }
}

impl FromSql<Integer, Pg> for PoolId
where
    i32: FromSql<Integer, Pg>,
{
    fn from_sql(bytes: <Pg as diesel::backend::Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        i32::from_sql(bytes).map(|id| PoolId { id })
    }
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(PoolCategory, foreign_key = category_id))]
#[diesel(table_name = pool)]
#[diesel(check_for_backend(Pg))]
pub struct Pool {
    pub id: i32,
    pub category_id: i32,
    pub description: String,
    pub creation_time: DateTime,
    pub last_edit_time: DateTime,
}

#[derive(Insertable)]
#[diesel(table_name = pool_name)]
pub struct NewPoolName<'a> {
    pub pool_id: i32,
    pub order: i32,
    pub name: &'a str,
}

#[derive(Serialize, Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(Pool))]
#[diesel(table_name = pool_name)]
#[diesel(check_for_backend(Pg))]
#[serde(transparent)]
pub struct PoolName {
    #[serde(skip)]
    pub id: i32,
    #[serde(skip)]
    pub pool_id: i32,
    #[serde(skip)]
    pub order: i32,
    pub name: String,
}

pub type NewPoolPost = PoolPost;

#[derive(Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Pool), belongs_to(Post))]
#[diesel(table_name = pool_post)]
#[diesel(primary_key(pool_id, post_id))]
#[diesel(check_for_backend(Pg))]
pub struct PoolPost {
    pub pool_id: i32,
    pub post_id: i32,
    pub order: i32,
}
