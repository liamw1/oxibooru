use crate::model::user::User;
use crate::schema::snapshot;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use std::option::Option;

#[derive(Insertable)]
#[diesel(table_name = snapshot)]
pub struct NewSnapshot<'a> {
    pub user_id: i32,
    pub resource_id: i32,
    pub resource_type: &'a str,
    pub resource_name: &'a str,
    pub operation: &'a str,
    pub creation_time: DateTime<Utc>,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(User))]
#[diesel(table_name = snapshot)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Snapshot {
    pub id: i32,
    pub user_id: Option<i32>,
    pub resource_id: i32,
    pub resource_type: String,
    pub resource_name: String,
    pub operation: String,
    pub data: Option<Vec<u8>>,
    pub creation_time: DateTime<Utc>,
}

impl Snapshot {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        snapshot::table.count().first(conn)
    }
}
