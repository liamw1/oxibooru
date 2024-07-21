use crate::api::{ApiResult, AuthResult, PagedQuery, PagedResponse, ResourceQuery};
use crate::auth::password;
use crate::model::enums::{AvatarStyle, UserRank};
use crate::model::user::{NewUser, User};
use crate::resource::user::{FieldTable, UserInfo, Visibility};
use crate::schema::user;
use crate::{api, config, resource, search};
use argon2::password_hash::SaltString;
use diesel::prelude::*;
use rand_core::OsRng;
use serde::Deserialize;
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
        .and(api::optional_query())
        .map(get_user)
        .map(api::Reply::from);
    let post_user = warp::post()
        .and(warp::path!("users"))
        .and(api::auth())
        .and(api::optional_query())
        .and(warp::body::json())
        .map(create_user)
        .map(api::Reply::from);

    list_users.or(get_user).or(post_user)
}

type PagedUserInfo = PagedResponse<UserInfo>;

const MAX_USERS_PER_PAGE: i64 = 50;

fn create_field_table(fields: Option<&str>) -> Result<FieldTable<bool>, Box<dyn std::error::Error>> {
    fields
        .map(resource::user::Field::create_table)
        .transpose()
        .map(|opt_table| opt_table.unwrap_or(FieldTable::filled(true)))
        .map_err(Box::from)
}

#[derive(Deserialize)]
struct NewUserInfo {
    name: String,
    password: String,
    email: Option<String>,
    rank: Option<UserRank>,
}

fn create_user(auth: AuthResult, query: ResourceQuery, user_info: NewUserInfo) -> ApiResult<UserInfo> {
    let client = auth?;
    let client_rank = api::client_access_level(client.as_ref());

    let creation_rank = user_info.rank.unwrap_or(UserRank::Regular);
    let required_rank = match client.is_some() {
        true => config::privileges().user_create_any,
        false => config::privileges().user_create_self,
    };

    api::verify_privilege(client.as_ref(), required_rank)?;
    let rank = creation_rank.clamp(UserRank::Regular, std::cmp::max(client_rank, UserRank::Regular));

    let fields = create_field_table(query.fields())?;

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
    UserInfo::new(&mut conn, user, &fields, Visibility::Full).map_err(api::Error::from)
}

fn get_user(username: String, auth: AuthResult, query: ResourceQuery) -> ApiResult<UserInfo> {
    let fields = create_field_table(query.fields())?;

    let mut conn = crate::establish_connection()?;
    let user = User::from_name(&mut conn, &username)?;

    let client = auth?;
    let client_id = client.as_ref().map(|user| user.id);
    if client_id != Some(user.id) {
        api::verify_privilege(client.as_ref(), config::privileges().user_view)?;
        return UserInfo::new(&mut conn, user, &fields, Visibility::PublicOnly).map_err(api::Error::from);
    }
    UserInfo::new(&mut conn, user, &fields, Visibility::Full).map_err(api::Error::from)
}

fn list_users(auth: AuthResult, query: PagedQuery) -> ApiResult<PagedUserInfo> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().user_list)?;

    let offset = query.offset.unwrap_or(0);
    let limit = std::cmp::min(query.limit, MAX_USERS_PER_PAGE);
    let fields = create_field_table(query.fields())?;

    let sql_query = search::user::build_query(query.criteria())?;
    println!("SQL Query: {}\n", diesel::debug_query::<diesel::pg::Pg, _>(&sql_query).to_string());

    let mut conn = crate::establish_connection()?;

    // Selecting by User for now, but would be more efficient to select by ids for large databases
    let users: Vec<User> = sql_query.select(User::as_select()).load(&mut conn)?;
    let total = users.len() as i64;
    let selected_posts: Vec<User> = users.into_iter().skip(offset as usize).take(limit as usize).collect();

    Ok(PagedUserInfo {
        query: query.query.query,
        offset,
        limit,
        total,
        results: UserInfo::new_batch(&mut conn, selected_posts, &fields, Visibility::PublicOnly)?,
    })
}
