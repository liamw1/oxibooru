use crate::api::AuthResult;
use crate::auth::password;
use crate::model::enums::{AvatarStyle, UserRank};
use crate::model::user::{NewUser, User};
use crate::schema::user;
use crate::util::DateTime;
use crate::{api, config};
use argon2::password_hash::SaltString;
use diesel::prelude::*;
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list_users = warp::get()
        .and(warp::path!("users"))
        .and(api::auth())
        .and(warp::query())
        .map(list_users)
        .map(api::Reply::from);
    let get_user = warp::get()
        .and(warp::path!("user" / String))
        .and(api::auth())
        .map(get_user)
        .map(api::Reply::from);
    let post_user = warp::post()
        .and(warp::path!("users"))
        .and(api::auth())
        .and(warp::body::json())
        .map(create_user)
        .map(api::Reply::from);

    list_users.or(get_user).or(post_user)
}

#[derive(Deserialize)]
struct NewUserInfo {
    name: String,
    password: String,
    email: Option<String>,
    rank: Option<UserRank>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum PrivateData<T> {
    Expose(T),
    Visible(bool), // Set to false to indicate hidden
}

// TODO: Remove renames by changing references to these names in client
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UserInfo {
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
type PagedUserInfo = api::PagedResponse<UserInfo>;

impl UserInfo {
    fn full(conn: &mut PgConnection, user: User) -> QueryResult<Self> {
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
    fn public_only(conn: &mut PgConnection, user: User) -> QueryResult<Self> {
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

fn create_user(auth_result: AuthResult, user_info: NewUserInfo) -> Result<UserInfo, api::Error> {
    let client = auth_result?;
    let client_rank = api::client_access_level(client.as_ref());

    let creation_rank = user_info.rank.unwrap_or(UserRank::Regular);
    let required_rank = match client.is_some() {
        true => config::privileges().user_create_any,
        false => config::privileges().user_create_self,
    };

    api::verify_privilege(client.as_ref(), required_rank)?;
    let rank = creation_rank.clamp(UserRank::Regular, std::cmp::max(client_rank, UserRank::Regular));

    let salt = SaltString::generate(&mut OsRng);
    let hash = password::hash_password(&user_info.password, salt.as_str())?;
    let new_user = NewUser {
        name: &user_info.name,
        password_hash: &hash,
        password_salt: salt.as_str(),
        email: user_info.email.as_deref(),
        rank,
        avatar_style: AvatarStyle::Gravatar,
    };

    let mut conn = crate::establish_connection()?;
    let user: User = diesel::insert_into(user::table)
        .values(&new_user)
        .returning(User::as_returning())
        .get_result(&mut conn)?;
    UserInfo::full(&mut conn, user).map_err(api::Error::from)
}

fn get_user(username: String, auth_result: AuthResult) -> Result<UserInfo, api::Error> {
    let mut conn = crate::establish_connection()?;
    let user = User::from_name(&mut conn, &username)?;

    let client = auth_result?;
    let client_id = client.as_ref().map(|user| user.id);
    if client_id != Some(user.id) {
        api::verify_privilege(client.as_ref(), config::privileges().user_view)?;
        return UserInfo::public_only(&mut conn, user).map_err(api::Error::from);
    }
    UserInfo::full(&mut conn, user).map_err(api::Error::from)
}

fn list_users(auth_result: AuthResult, query_info: api::PagedQuery) -> Result<PagedUserInfo, api::Error> {
    let client = auth_result?;
    api::verify_privilege(client.as_ref(), config::privileges().user_list)?;

    let offset = query_info.offset.unwrap_or(0);
    let limit = query_info.limit;

    let mut conn = crate::establish_connection()?;
    let users = user::table
        .select(User::as_select())
        .limit(limit)
        .offset(offset)
        .load(&mut conn)?;

    Ok(PagedUserInfo {
        query: query_info.query.unwrap_or(String::new()),
        offset,
        limit,
        total: User::count(&mut conn)?,
        results: users
            .into_iter()
            .map(|user| UserInfo::public_only(&mut conn, user))
            .collect::<QueryResult<_>>()?,
    })
}
