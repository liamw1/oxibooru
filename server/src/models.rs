use crate::schema::users;
use chrono::DateTime;
use chrono::Utc;
use diesel::prelude::*;
use std::option::Option;

#[derive(Queryable, Selectable)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub id: i32,
    pub name: String,
    pub password_hash: String,
    pub password_salt: Option<String>,
    pub email: Option<String>,
    pub rank: String,
    pub creation_time: DateTime<Utc>,
    pub last_login_time: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = users)]
pub struct NewUser<'a> {
    pub name: &'a str,
    pub password_hash: &'a str,
    pub rank: &'a str,
    pub creation_time: DateTime<Utc>,
    pub last_login_time: DateTime<Utc>,
}
