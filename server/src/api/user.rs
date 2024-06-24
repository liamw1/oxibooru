use crate::api;
use crate::auth::password;
use crate::model::enums::UserRank;
use crate::model::user::{NewUser, User};
use crate::schema::user;
use crate::util::DateTime;
use argon2::password_hash::SaltString;
use diesel::prelude::*;
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use warp::hyper::body::Bytes;
use warp::reject::Rejection;

pub async fn list_users(auth_result: api::AuthenticationResult, body: Bytes) -> Result<api::Reply, Rejection> {
    Ok(auth_result
        .and_then(|client| api::parse_body(&body).and_then(|request| read_users(request, client.as_ref())))
        .into())
}

pub async fn get_user(username: String, auth_result: api::AuthenticationResult) -> Result<api::Reply, Rejection> {
    Ok(auth_result
        .and_then(|client| read_user(username, client.as_ref()))
        .into())
}

pub async fn post_user(auth_result: api::AuthenticationResult, body: Bytes) -> Result<api::Reply, Rejection> {
    Ok(auth_result
        .and_then(|client| api::parse_body(&body).and_then(|user_info| create_user(user_info, client.as_ref())))
        .into())
}

#[derive(Deserialize)]
struct NewUserInfo {
    name: String,
    password: String,
    email: Option<String>,
    rank: Option<UserRank>,
}

// TODO: Remove renames by changing references to these names in client
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UserInfo {
    version: DateTime,
    name: String,
    email: Option<String>,
    rank: UserRank,
    last_login_time: DateTime,
    creation_time: DateTime,
    avatar_style: String,
    avatar_url: String,
    comment_count: i64,
    uploaded_post_count: i64,
    liked_post_count: String,
    disliked_post_count: String,
    favorite_post_count: i64,
}
type PagedUserInfo = api::PagedResponse<UserInfo>;

impl UserInfo {
    fn full(conn: &mut PgConnection, user: User) -> Result<Self, api::Error> {
        let avatar_url = user.avatar_url();
        let comment_count = user.comment_count(conn)?;
        let uploaded_post_count = user.post_count(conn)?;
        let liked_post_count = user.liked_post_count(conn)?;
        let disliked_post_count = user.disliked_post_count(conn)?;
        let favorite_post_count = user.favorite_post_count(conn)?;

        Ok(Self {
            version: user.last_edit_time,
            name: user.name,
            email: user.email,
            rank: user.rank,
            last_login_time: user.last_login_time,
            creation_time: user.creation_time,
            avatar_url,
            avatar_style: user.avatar_style,
            comment_count,
            uploaded_post_count,
            liked_post_count: liked_post_count.to_string(),
            disliked_post_count: disliked_post_count.to_string(),
            favorite_post_count,
        })
    }

    // Returns a subset of the information available about a user
    fn public_only(conn: &mut PgConnection, user: User) -> Result<Self, api::Error> {
        let avatar_url = user.avatar_url();
        let comment_count = user.comment_count(conn)?;
        let uploaded_post_count = user.post_count(conn)?;
        let favorite_post_count = user.favorite_post_count(conn)?;

        const HIDDEN: &'static str = "false";
        Ok(Self {
            version: user.last_edit_time,
            name: user.name,
            email: Some(HIDDEN.to_owned()),
            rank: user.rank,
            last_login_time: user.last_login_time,
            creation_time: user.creation_time,
            avatar_url,
            avatar_style: user.avatar_style,
            comment_count,
            uploaded_post_count,
            liked_post_count: HIDDEN.to_owned(),
            disliked_post_count: HIDDEN.to_owned(),
            favorite_post_count,
        })
    }
}

fn create_user(user_info: NewUserInfo, client: Option<&User>) -> Result<UserInfo, api::Error> {
    let target = if client.is_some() { "any" } else { "self" };
    let client_rank = api::client_access_level(client);
    let requested_rank = user_info.rank.unwrap_or(UserRank::Regular);
    let requested_action = "users:create:".to_owned() + target;

    api::verify_privilege(client_rank, &requested_action)?;
    let rank = requested_rank.clamp(UserRank::Regular, std::cmp::max(client_rank, UserRank::Regular));

    let salt = SaltString::generate(&mut OsRng);
    let hash = password::hash_password(&user_info.password, salt.as_str())?;
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
        .get_result(&mut conn)?;
    UserInfo::full(&mut conn, user)
}

fn read_user(username: String, client: Option<&User>) -> Result<UserInfo, api::Error> {
    let mut conn = crate::establish_connection()?;
    let user = User::from_name(&mut conn, &username)?;

    let client_id = client.map(|user| user.id);
    if client_id != Some(user.id) {
        api::verify_privilege(api::client_access_level(client), "users:view")?;
        return UserInfo::public_only(&mut conn, user);
    }
    UserInfo::full(&mut conn, user)
}

fn read_users(body: api::PagedRequest, client: Option<&User>) -> Result<PagedUserInfo, api::Error> {
    api::verify_privilege(api::client_access_level(client), "users:list")?;

    let offset = body.offset.unwrap_or(0);
    let limit = body.limit.unwrap_or(40);

    let mut conn = crate::establish_connection()?;
    let users = user::table
        .select(User::as_select())
        .limit(limit)
        .offset(offset)
        .load(&mut conn)?;

    Ok(PagedUserInfo {
        query: body.query,
        offset,
        limit,
        total: users.len() as i64,
        results: users
            .into_iter()
            .map(|user| UserInfo::public_only(&mut conn, user))
            .collect::<Result<_, _>>()?,
    })
}
