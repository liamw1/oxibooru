use crate::schema::pool;
use crate::schema::pool_category;
use crate::schema::pool_name;
use crate::schema::pool_post;
use chrono::DateTime;
use chrono::Utc;
use diesel::prelude::*;
use std::option::Option;

#[derive(Insertable)]
#[diesel(table_name = pool_category)]
pub struct NewPoolCategory<'a> {
    pub name: &'a str,
    pub color: &'a str,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = pool_category)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PoolCategory {
    pub id: i32,
    pub name: String,
    pub color: String,
}

#[derive(Insertable)]
#[diesel(table_name = pool)]
pub struct NewPool {
    pub category_id: i32,
    pub creation_time: DateTime<Utc>,
    pub last_edit_time: DateTime<Utc>,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = pool)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Pool {
    pub id: i32,
    pub category_id: i32,
    pub description: Option<String>,
    pub creation_time: DateTime<Utc>,
    pub last_edit_time: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = pool_name)]
pub struct NewPoolName<'a> {
    pub pool_id: i32,
    pub order: i32,
    pub name: &'a str,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = pool_name)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PoolName {
    pub id: i32,
    pub pool_id: i32,
    pub order: i32,
    pub name: String,
}

#[allow(dead_code)]
type NewPoolPost = PoolPost;

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = pool_post)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PoolPost {
    pub pool_id: i32,
    pub post_id: i32,
    pub order: i32,
}
