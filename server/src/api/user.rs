use crate::api::{ApiResult, AuthResult, DeleteRequest, PagedQuery, PagedResponse, ResourceQuery};
use crate::auth::password;
use crate::config::RegexType;
use crate::model::enums::{AvatarStyle, UserRank};
use crate::model::user::{NewUser, User};
use crate::resource::user::{FieldTable, UserInfo, Visibility};
use crate::schema::user;
use crate::util::DateTime;
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
        .and(api::resource_query())
        .map(get_user)
        .map(api::Reply::from);
    let create_user = warp::post()
        .and(warp::path!("users"))
        .and(api::auth())
        .and(api::resource_query())
        .and(warp::body::json())
        .map(create_user)
        .map(api::Reply::from);
    let update_user = warp::put()
        .and(warp::path!("user" / String))
        .and(api::auth())
        .and(api::resource_query())
        .and(warp::body::json())
        .map(update_user)
        .map(api::Reply::from);
    let delete_user = warp::delete()
        .and(warp::path!("user" / String))
        .and(api::auth())
        .and(warp::body::json())
        .map(delete_user)
        .map(api::Reply::from);

    list_users.or(get_user).or(create_user).or(update_user).or(delete_user)
}

const MAX_USERS_PER_PAGE: i64 = 50;

fn create_field_table(fields: Option<&str>) -> Result<FieldTable<bool>, Box<dyn std::error::Error>> {
    fields
        .map(resource::user::Field::create_table)
        .transpose()
        .map(|opt_table| opt_table.unwrap_or(FieldTable::filled(true)))
        .map_err(Box::from)
}

fn list_users(auth: AuthResult, query: PagedQuery) -> ApiResult<PagedResponse<UserInfo>> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().user_list)?;

    let offset = query.offset.unwrap_or(0);
    let limit = std::cmp::min(query.limit.get(), MAX_USERS_PER_PAGE);
    let fields = create_field_table(query.fields())?;

    crate::establish_connection()?.transaction(|conn| {
        let mut search_criteria = search::user::parse_search_criteria(query.criteria())?;
        search_criteria.add_offset_and_limit(offset, limit);
        let count_query = search::user::build_query(&search_criteria)?;
        let sql_query = search::user::build_query(&search_criteria)?;

        let total = count_query.count().first(conn)?;
        let selected_users: Vec<i32> = search::user::get_ordered_ids(conn, sql_query, &search_criteria)?;
        Ok(PagedResponse {
            query: query.query.query,
            offset,
            limit,
            total,
            results: UserInfo::new_batch_from_ids(conn, selected_users, &fields, Visibility::PublicOnly)?,
        })
    })
}

fn get_user(username: String, auth: AuthResult, query: ResourceQuery) -> ApiResult<UserInfo> {
    let client = auth?;
    let client_id = client.as_ref().map(|user| user.id);
    let fields = create_field_table(query.fields())?;
    let username = percent_encoding::percent_decode_str(&username).decode_utf8()?;

    crate::establish_connection()?.transaction(|conn| {
        let user = User::from_name(conn, &username)?;

        let viewing_self = client_id == Some(user.id);
        let visibility = match viewing_self {
            true => Visibility::Full,
            false => Visibility::PublicOnly,
        };

        if !viewing_self {
            api::verify_privilege(client.as_ref(), config::privileges().user_view)?;
        }
        UserInfo::new(conn, user, &fields, visibility).map_err(api::Error::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewUserInfo {
    name: String,
    password: String,
    email: Option<String>,
    rank: Option<UserRank>,
}

fn create_user(auth: AuthResult, query: ResourceQuery, user_info: NewUserInfo) -> ApiResult<UserInfo> {
    let client = auth?;

    let creation_rank = user_info.rank.unwrap_or(config::default_rank());
    let required_rank = match client.is_some() {
        true => config::privileges().user_create_any,
        false => config::privileges().user_create_self,
    };
    api::verify_privilege(client.as_ref(), required_rank)?;
    if creation_rank > config::default_rank() {
        api::verify_privilege(client.as_ref(), creation_rank)?;
    }

    let fields = create_field_table(query.fields())?;
    api::verify_matches_regex(&user_info.name, RegexType::Username)?;
    api::verify_matches_regex(&user_info.password, RegexType::Password)?;

    let salt = SaltString::generate(&mut OsRng);
    let hash = password::hash_password(&user_info.password, salt.as_str())?;
    let new_user = NewUser {
        name: &user_info.name,
        password_hash: &hash,
        password_salt: salt.as_str(),
        email: user_info.email.as_deref(),
        rank: creation_rank,
        avatar_style: AvatarStyle::Gravatar,
    };

    crate::establish_connection()?.transaction(|conn| {
        let user: User = diesel::insert_into(user::table)
            .values(new_user)
            .returning(User::as_returning())
            .get_result(conn)?;
        UserInfo::new(conn, user, &fields, Visibility::Full).map_err(api::Error::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct UserUpdate {
    version: DateTime,
    name: Option<String>,
    password: Option<String>,
    email: Option<String>,
    rank: Option<UserRank>,
    avatar_style: Option<AvatarStyle>,
}

fn update_user(username: String, auth: AuthResult, query: ResourceQuery, update: UserUpdate) -> ApiResult<UserInfo> {
    let client = auth?;
    let client_id = client.as_ref().map(|user| user.id);
    let fields = create_field_table(query.fields())?;
    let username = percent_encoding::percent_decode_str(&username).decode_utf8()?;

    crate::establish_connection()?.transaction(|conn| {
        let (user_id, user_version): (i32, DateTime) = user::table
            .select((user::id, user::last_edit_time))
            .filter(user::name.eq(username))
            .first(conn)?;
        api::verify_version(user_version, update.version)?;

        let editing_self = client_id == Some(user_id);
        let visibility = match editing_self {
            true => Visibility::Full,
            false => Visibility::PublicOnly,
        };

        if let Some(name) = update.name {
            let required_rank = match editing_self {
                true => config::privileges().user_edit_self_name,
                false => config::privileges().user_edit_any_name,
            };
            api::verify_privilege(client.as_ref(), required_rank)?;
            api::verify_matches_regex(&name, RegexType::Username)?;

            diesel::update(user::table.find(user_id))
                .set(user::name.eq(name))
                .execute(conn)?;
        }
        if let Some(password) = update.password {
            let required_rank = match editing_self {
                true => config::privileges().user_edit_self_pass,
                false => config::privileges().user_edit_any_pass,
            };
            api::verify_privilege(client.as_ref(), required_rank)?;
            api::verify_matches_regex(&password, RegexType::Password)?;

            let salt = SaltString::generate(&mut OsRng);
            let hash = password::hash_password(&password, salt.as_str())?;
            diesel::update(user::table.find(user_id))
                .set(user::password_salt.eq(salt.as_str()))
                .execute(conn)?;
            diesel::update(user::table.find(user_id))
                .set(user::password_hash.eq(hash))
                .execute(conn)?;
        }
        if let Some(email) = update.email {
            let required_rank = match editing_self {
                true => config::privileges().user_edit_self_email,
                false => config::privileges().user_edit_any_email,
            };
            api::verify_privilege(client.as_ref(), required_rank)?;

            diesel::update(user::table.find(user_id))
                .set(user::email.eq(email))
                .execute(conn)?;
        }
        if let Some(rank) = update.rank {
            let required_rank = match editing_self {
                true => config::privileges().user_edit_self_rank,
                false => config::privileges().user_edit_any_rank,
            };
            api::verify_privilege(client.as_ref(), required_rank)?;
            if rank > config::default_rank() {
                api::verify_privilege(client.as_ref(), rank)?;
            }

            diesel::update(user::table.find(user_id))
                .set(user::rank.eq(rank))
                .execute(conn)?;
        }
        if let Some(avatar_style) = update.avatar_style {
            let required_rank = match editing_self {
                true => config::privileges().user_edit_self_avatar,
                false => config::privileges().user_edit_any_avatar,
            };
            api::verify_privilege(client.as_ref(), required_rank)?;

            diesel::update(user::table.find(user_id))
                .set(user::avatar_style.eq(avatar_style))
                .execute(conn)?;
        }

        UserInfo::new_from_id(conn, user_id, &fields, visibility).map_err(api::Error::from)
    })
}

fn delete_user(username: String, auth: AuthResult, client_version: DeleteRequest) -> ApiResult<()> {
    let client = auth?;
    let client_id = client.as_ref().map(|user| user.id);
    let username = percent_encoding::percent_decode_str(&username).decode_utf8()?;

    crate::establish_connection()?.transaction(|conn| {
        let (user_id, user_version): (i32, DateTime) = user::table
            .select((user::id, user::last_edit_time))
            .filter(user::name.eq(username))
            .first(conn)?;
        api::verify_version(user_version, *client_version)?;

        let deleting_self = client_id == Some(user_id);
        let required_rank = match deleting_self {
            true => config::privileges().user_delete_self,
            false => config::privileges().user_delete_any,
        };
        api::verify_privilege(client.as_ref(), required_rank)?;

        diesel::delete(user::table.find(user_id)).execute(conn)?;
        Ok(())
    })
}
