use crate::api;
use crate::api::ApiError;
use crate::api::AuthenticationResult;
use crate::api::Reply;
use crate::auth::hash;
use crate::model::rank::UserRank;
use crate::model::user::{NewUser, User};
use crate::schema::user;
use argon2::password_hash::SaltString;
use diesel::prelude::*;
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use warp::hyper::body::Bytes;
use warp::reject::Rejection;

pub async fn get_user(username: String, auth_result: AuthenticationResult) -> Result<Reply, Rejection> {
    Ok(Reply::from(auth_result.and_then(|client| read_user(username, client.as_ref()))))
}

pub async fn post_user(auth_result: AuthenticationResult, body: Bytes) -> Result<Reply, Rejection> {
    Ok(Reply::from(auth_result.and_then(|client| {
        api::parse_body(&body).and_then(|user_info| create_user(user_info, client.as_ref()))
    })))
}

#[derive(Deserialize)]
struct NewUserInfo {
    name: String,
    password: String,
    email: Option<String>,
    rank: Option<String>,
}

// TODO: Remove renames by changing references to these names in client
#[derive(Serialize)]
struct UserInfo {
    version: i32,
    name: String,
    email: Option<String>,
    rank: String,
    #[serde(rename(serialize = "lastLogintime"))]
    last_login_time: String,
    #[serde(rename(serialize = "creationTime"))]
    creation_time: String,
    #[serde(rename(serialize = "avatarStyle"))]
    avatar_style: String,
    #[serde(rename(serialize = "avatarUrl"))]
    avatar_url: String,
    #[serde(rename(serialize = "commentCount"))]
    comment_count: i64,
    #[serde(rename(serialize = "uploadedPostCount"))]
    uploaded_post_count: i64,
    #[serde(rename(serialize = "likedPostCount"))]
    liked_post_count: i64,
    #[serde(rename(serialize = "dislikedPostCount"))]
    disliked_post_count: i64,
    #[serde(rename(serialize = "favoritePostCount"))]
    favorite_post_count: i64,
}

impl UserInfo {
    fn new(conn: &mut PgConnection, user: User) -> Result<UserInfo, ApiError> {
        let comment_count = user.comment_count(conn)?;
        let uploaded_post_count = user.post_count(conn)?;
        let liked_post_count = user.liked_post_count(conn)?;
        let disliked_post_count = user.disliked_post_count(conn)?;
        let favorite_post_count = user.favorite_post_count(conn)?;

        Ok(UserInfo {
            version: 0,
            name: user.name,
            email: user.email,
            rank: user.rank.to_string(),
            last_login_time: user.last_login_time.to_string(),
            creation_time: user.creation_time.to_string(),
            avatar_url: String::new(),
            avatar_style: String::new(),
            comment_count,
            uploaded_post_count,
            liked_post_count,
            disliked_post_count,
            favorite_post_count,
        })
    }
}

fn create_user(user_info: NewUserInfo, client: Option<&User>) -> Result<UserInfo, ApiError> {
    let requested_rank = match user_info.rank {
        Some(rank) => UserRank::from_str(&rank)?,
        None => UserRank::Regular,
    };
    let rank = match client {
        Some(client_user) => match client_user.rank.has_permission_to("users:create:any") {
            true => requested_rank.clamp(UserRank::Restricted, client_user.rank),
            false => return Err(ApiError::InsufficientPrivileges),
        },
        None => match UserRank::Anonymous.has_permission_to("users:create:self") {
            true => UserRank::Regular,
            false => return Err(ApiError::InsufficientPrivileges),
        },
    };

    let salt = SaltString::generate(&mut OsRng);
    let hash = hash::hash_password(&user_info.password, salt.as_str())?;
    let new_user = NewUser {
        name: &user_info.name,
        password_hash: &hash,
        password_salt: salt.as_str(),
        email: user_info.email.as_deref(),
        rank,
    };

    let mut conn = crate::establish_connection()?;
    let user: User = diesel::insert_into(user::table)
        .values(&new_user)
        .returning(User::as_returning())
        .get_result(&mut conn)
        .map_err(ApiError::from)?;
    UserInfo::new(&mut conn, user)
}

// NOTE: Should we query by user_id instead?
fn read_user(username: String, client: Option<&User>) -> Result<UserInfo, ApiError> {
    let mut conn = crate::establish_connection()?;
    let user = User::from_name(&mut conn, &username)?;

    let access_level = api::client_access_level(client);
    let client_id = client.map(|user| user.id);
    if client_id != Some(user.id) && !access_level.has_permission_to("users:view") {
        return Err(ApiError::InsufficientPrivileges);
    }

    UserInfo::new(&mut conn, user)
}
