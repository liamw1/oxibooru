use crate::auth::content;
use crate::model::enums::{AvatarStyle, UserRank};
use crate::model::user::User;
use crate::util::DateTime;
use diesel::prelude::*;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicroUser {
    name: String,
    avatar_url: String,
}

impl MicroUser {
    pub fn new(user: User) -> Self {
        let avatar_url = user.avatar_url();
        Self {
            name: user.name,
            avatar_url,
        }
    }

    pub fn new2(name: String, avatar_style: AvatarStyle) -> Self {
        let avatar_url = match avatar_style {
            AvatarStyle::Gravatar => content::gravatar_url(&name),
            AvatarStyle::Manual => content::custom_avatar_url(&name),
        };
        Self { name, avatar_url }
    }
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum PrivateData<T> {
    Expose(T),
    Visible(bool), // Set to false to indicate hidden
}

// TODO: Remove renames by changing references to these names in client
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    version: DateTime,
    name: String,
    email: PrivateData<Option<String>>,
    rank: UserRank,
    last_login_time: DateTime,
    creation_time: DateTime,
    avatar_style: AvatarStyle,
    avatar_url: String,
    comment_count: i64,
    uploaded_post_count: i64,
    liked_post_count: PrivateData<i64>,
    disliked_post_count: PrivateData<i64>,
    favorite_post_count: i64,
}

impl UserInfo {
    pub fn full(conn: &mut PgConnection, user: User) -> QueryResult<Self> {
        let avatar_url = user.avatar_url();
        let comment_count = user.comment_count(conn)?;
        let uploaded_post_count = user.post_count(conn)?;
        let liked_post_count = user.liked_post_count(conn)?;
        let disliked_post_count = user.disliked_post_count(conn)?;
        let favorite_post_count = user.favorite_post_count(conn)?;

        Ok(Self {
            version: user.last_edit_time,
            name: user.name,
            email: PrivateData::Expose(user.email),
            rank: user.rank,
            last_login_time: user.last_login_time,
            creation_time: user.creation_time,
            avatar_url,
            avatar_style: user.avatar_style,
            comment_count,
            uploaded_post_count,
            liked_post_count: PrivateData::Expose(liked_post_count),
            disliked_post_count: PrivateData::Expose(disliked_post_count),
            favorite_post_count,
        })
    }

    // Returns a subset of the information available about a user
    pub fn public_only(conn: &mut PgConnection, user: User) -> QueryResult<Self> {
        let avatar_url = user.avatar_url();
        let comment_count = user.comment_count(conn)?;
        let uploaded_post_count = user.post_count(conn)?;
        let favorite_post_count = user.favorite_post_count(conn)?;

        Ok(Self {
            version: user.last_edit_time,
            name: user.name,
            email: PrivateData::Visible(false),
            rank: user.rank,
            last_login_time: user.last_login_time,
            creation_time: user.creation_time,
            avatar_url,
            avatar_style: user.avatar_style,
            comment_count,
            uploaded_post_count,
            liked_post_count: PrivateData::Visible(false),
            disliked_post_count: PrivateData::Visible(false),
            favorite_post_count,
        })
    }
}
