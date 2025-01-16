use crate::model::post::Post;
use crate::schema::{pool, pool_category, pool_name, pool_post};
use crate::time::DateTime;
use diesel::pg::Pg;
use diesel::prelude::*;

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
    pub id: i32,
    pub name: String,
    pub color: String,
    pub last_edit_time: DateTime,
}

#[derive(Insertable)]
#[diesel(table_name = pool)]
#[diesel(check_for_backend(Pg))]
pub struct NewPool<'a> {
    pub category_id: i32,
    pub description: &'a str,
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
#[diesel(check_for_backend(Pg))]
pub struct NewPoolName<'a> {
    pub pool_id: i32,
    pub order: i32,
    pub name: &'a str,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(Pool))]
#[diesel(table_name = pool_name)]
#[diesel(primary_key(pool_id, order))]
#[diesel(check_for_backend(Pg))]
pub struct PoolName {
    pub pool_id: i32,
    pub order: i32,
    pub name: String,
}

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
