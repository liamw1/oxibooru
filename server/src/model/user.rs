use crate::schema::user;
use crate::schema::user_token;
use chrono::DateTime;
use chrono::Utc;
use diesel::prelude::*;
use std::option::Option;

#[derive(Insertable)]
#[diesel(table_name = user)]
pub struct NewUser<'a> {
    pub name: &'a str,
    pub password_hash: &'a str,
    pub rank: &'a str,
    pub creation_time: DateTime<Utc>,
    pub last_login_time: DateTime<Utc>,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = user)]
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
#[diesel(table_name = user_token)]
pub struct NewUserToken<'a> {
    pub user_id: i32,
    pub token: &'a str,
    pub enabled: bool,
    pub expiration_time: Option<DateTime<Utc>>,
    pub creation_time: DateTime<Utc>,
    pub last_edit_time: DateTime<Utc>,
    pub last_usage_time: DateTime<Utc>,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = user_token)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UserToken {
    pub user_id: i32,
    pub token: String,
    pub note: Option<String>,
    pub enabled: bool,
    pub expiration_time: Option<DateTime<Utc>>,
    pub creation_time: DateTime<Utc>,
    pub last_edit_time: DateTime<Utc>,
    pub last_usage_time: DateTime<Utc>,
}
