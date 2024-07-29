use crate::model::post::Post;
use crate::model::IntegerIdentifiable;
use crate::schema::{pool, pool_category, pool_name, pool_post};
use crate::util::DateTime;
use diesel::pg::Pg;
use diesel::prelude::*;
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

impl IntegerIdentifiable for Pool {
    fn id(&self) -> i32 {
        self.id
    }
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
#[diesel(primary_key(pool_id, order))]
#[diesel(check_for_backend(Pg))]
#[serde(transparent)]
pub struct PoolName {
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
