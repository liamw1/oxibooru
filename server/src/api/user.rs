use crate::api;
use crate::api::ApiError;
use crate::api::Reply;
use crate::auth::hash;
use crate::model::rank::UserRank;
use crate::model::user::{NewUser, User};
use crate::schema::user;
use argon2::password_hash::SaltString;
use diesel::prelude::*;
use rand_core::OsRng;
use serde::Serialize;
use warp::hyper::body::Bytes;
use warp::reject::Rejection;

pub async fn post_user(body: Bytes) -> Result<Reply, Rejection> {
    Ok(Reply::from(api::parse_body(body).and_then(|json| create_user(json))))
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

fn create_user(json: serde_json::Value) -> Result<UserInfo, ApiError> {
    // TODO: Implement for non-anonymous users
    if !UserRank::Anonymous.has_permission_to("users:create:self") {
        return Err(ApiError::InsufficientPrivileges);
    }

    let name = json.get("name").ok_or(ApiError::MissingBodyParam("name"))?;
    let password = json.get("password").ok_or(ApiError::MissingBodyParam("password"))?;
    let email = json.get("email").map(|val| val.to_string());

    let salt = SaltString::generate(&mut OsRng);
    let hash = hash::hash_password(&password.to_string(), salt.as_str())?;
    let new_user = NewUser {
        name: &name.to_string(),
        password_hash: &hash,
        password_salt: salt.as_str(),
        email: email.as_deref(),
        rank: UserRank::Regular,
    };

    let mut conn = crate::establish_connection()?;
    let user: User = diesel::insert_into(user::table)
        .values(&new_user)
        .returning(User::as_returning())
        .get_result(&mut conn)
        .map_err(ApiError::from)?;
    let comment_count = user.comment_count(&mut conn)?;
    let uploaded_post_count = user.post_count(&mut conn)?;
    let liked_post_count = user.liked_post_count(&mut conn)?;
    let disliked_post_count = user.disliked_post_count(&mut conn)?;
    let favorite_post_count = user.favorite_post_count(&mut conn)?;

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
