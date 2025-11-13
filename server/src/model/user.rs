use crate::content::hash;
use crate::model::enums::{AvatarStyle, UserRank};
use crate::schema::{user, user_token};
use crate::string::{LargeString, SmallString};
use crate::time::DateTime;
use diesel::pg::Pg;
use diesel::prelude::*;
use std::option::Option;
use uuid::Uuid;

#[derive(Insertable)]
#[diesel(table_name = user)]
#[diesel(check_for_backend(Pg))]
pub struct NewUser<'a> {
    pub name: &'a str,
    pub password_hash: &'a str,
    pub password_salt: &'a str,
    pub email: Option<&'a str>,
    pub rank: UserRank,
    pub avatar_style: AvatarStyle,
}

#[derive(Identifiable, Queryable, Selectable)]
#[diesel(table_name = user)]
#[diesel(check_for_backend(Pg))]
pub struct User {
    pub id: i64,
    pub name: SmallString,
    pub password_hash: String,
    pub password_salt: String,
    pub email: Option<SmallString>,
    pub rank: UserRank,
    pub avatar_style: AvatarStyle,
    pub creation_time: DateTime,
    pub last_login_time: DateTime,
    pub last_edit_time: DateTime,
    #[allow(dead_code)]
    custom_avatar_size: i64,
    #[allow(dead_code)]
    search_seed: f32,
}

impl User {
    /// Returns a URL to either a custom or gravatar avatar depending on the user's avatar style.
    pub fn avatar_url(&self) -> String {
        match self.avatar_style {
            AvatarStyle::Gravatar => hash::gravatar_url(&self.name),
            AvatarStyle::Manual => hash::custom_avatar_url(&self.name),
        }
    }
}

#[derive(Insertable)]
#[diesel(table_name = user_token)]
#[diesel(check_for_backend(Pg))]
pub struct NewUserToken<'a> {
    pub id: Uuid,
    pub user_id: i64,
    pub note: Option<&'a str>,
    pub enabled: bool,
    pub expiration_time: Option<DateTime>,
}

#[derive(AsChangeset, Associations, Identifiable, Queryable, Selectable)]
#[diesel(treat_none_as_null = true)]
#[diesel(belongs_to(User))]
#[diesel(table_name = user_token)]
#[diesel(check_for_backend(Pg))]
pub struct UserToken {
    pub id: Uuid,
    pub user_id: i64,
    pub note: LargeString,
    pub enabled: bool,
    pub expiration_time: Option<DateTime>,
    pub creation_time: DateTime,
    pub last_edit_time: DateTime,
    pub last_usage_time: DateTime,
}
