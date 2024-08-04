use crate::model::user::User;
use crate::schema::snapshot;
use crate::util::DateTime;
use diesel::pg::Pg;
use diesel::prelude::*;
use std::option::Option;

#[derive(Insertable)]
#[diesel(table_name = snapshot)]
#[diesel(check_for_backend(Pg))]
pub struct NewSnapshot<'a> {
    pub user_id: i32,
    pub resource_id: i32,
    pub resource_type: &'a str,
    pub resource_name: &'a str,
    pub operation: &'a str,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(User))]
#[diesel(table_name = snapshot)]
#[diesel(check_for_backend(Pg))]
pub struct Snapshot {
    pub id: i32,
    pub user_id: Option<i32>,
    pub resource_id: i32,
    pub resource_type: String,
    pub resource_name: String,
    pub operation: String,
    pub data: Option<Vec<u8>>,
    pub creation_time: DateTime,
}
