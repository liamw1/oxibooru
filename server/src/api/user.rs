use crate::api::{ApiResult, AuthResult, DeleteRequest, PagedQuery, PagedResponse, ResourceQuery};
use crate::auth::password;
use crate::config::RegexType;
use crate::content::hash;
use crate::content::thumbnail::{self, ThumbnailType};
use crate::model::enums::{AvatarStyle, ResourceType, UserRank};
use crate::model::user::NewUser;
use crate::resource::user::{FieldTable, UserInfo, Visibility};
use crate::schema::{database_statistics, user};
use crate::time::DateTime;
use crate::{api, config, db, filesystem, resource, search};
use argon2::password_hash::SaltString;
use diesel::prelude::*;
use rand_core::OsRng;
use serde::Deserialize;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list_users = warp::get()
        .and(api::auth())
        .and(warp::path!("users"))
        .and(warp::query())
        .map(list_users)
        .map(api::Reply::from);
    let get_user = warp::get()
        .and(api::auth())
        .and(warp::path!("user" / String))
        .and(api::resource_query())
        .map(get_user)
        .map(api::Reply::from);
    let create_user = warp::post()
        .and(api::auth())
        .and(warp::path!("users"))
        .and(api::resource_query())
        .and(warp::body::json())
        .map(create_user)
        .map(api::Reply::from);
    let update_user = warp::put()
        .and(api::auth())
        .and(warp::path!("user" / String))
        .and(api::resource_query())
        .and(warp::body::json())
        .map(update_user)
        .map(api::Reply::from);
    let delete_user = warp::delete()
        .and(api::auth())
        .and(warp::path!("user" / String))
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
    query.bump_login(client)?;
    api::verify_privilege(client, config::privileges().user_list)?;

    let offset = query.offset.unwrap_or(0);
    let limit = std::cmp::min(query.limit.get(), MAX_USERS_PER_PAGE);
    let fields = create_field_table(query.fields())?;

    db::get_connection()?.transaction(|conn| {
        let mut search_criteria = search::user::parse_search_criteria(query.criteria())?;
        search_criteria.add_offset_and_limit(offset, limit);
        let sql_query = search::user::build_query(&search_criteria)?;

        let total = if search_criteria.has_filter() {
            let count_query = search::user::build_query(&search_criteria)?;
            count_query.count().first(conn)?
        } else {
            database_statistics::table
                .select(database_statistics::user_count)
                .first(conn)?
        };

        let selected_users: Vec<i64> = search::user::get_ordered_ids(conn, sql_query, &search_criteria)?;
        Ok(PagedResponse {
            query: query.query.query,
            offset,
            limit,
            total,
            results: UserInfo::new_batch_from_ids(conn, selected_users, &fields, Visibility::PublicOnly)?,
        })
    })
}

fn get_user(auth: AuthResult, username: String, query: ResourceQuery) -> ApiResult<UserInfo> {
    let client = auth?;
    query.bump_login(client)?;

    let client_id = client.map(|user| user.id);
    let fields = create_field_table(query.fields())?;
    let username = percent_encoding::percent_decode_str(&username).decode_utf8()?;

    db::get_connection()?.transaction(|conn| {
        let user_id = user::table
            .select(user::id)
            .filter(user::name.eq(username))
            .first(conn)
            .optional()?
            .ok_or(api::Error::NotFound(ResourceType::User))?;

        let viewing_self = client_id == Some(user_id);
        if !viewing_self {
            api::verify_privilege(client, config::privileges().user_view)?;
        }

        let visibility = match viewing_self {
            true => Visibility::Full,
            false => Visibility::PublicOnly,
        };
        UserInfo::new_from_id(conn, user_id, &fields, visibility).map_err(api::Error::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct NewUserInfo {
    name: String,
    password: String,
    email: Option<String>,
    rank: Option<UserRank>,
    avatar_style: Option<AvatarStyle>,
    avatar_token: Option<String>,
}

fn create_user(auth: AuthResult, query: ResourceQuery, user_info: NewUserInfo) -> ApiResult<UserInfo> {
    let client = auth?;
    query.bump_login(client)?;

    let creation_rank = user_info.rank.unwrap_or(config::default_rank());
    if creation_rank == UserRank::Anonymous {
        return Err(api::Error::InvalidUserRank);
    }

    let required_rank = match client.is_some() {
        true => config::privileges().user_create_any,
        false => config::privileges().user_create_self,
    };
    api::verify_privilege(client, required_rank)?;
    if creation_rank > config::default_rank() {
        api::verify_privilege(client, creation_rank)?;
    }

    let fields = create_field_table(query.fields())?;
    api::verify_matches_regex(&user_info.name, RegexType::Username)?;
    api::verify_matches_regex(&user_info.password, RegexType::Password)?;
    api::verify_valid_email(user_info.email.as_deref())?;

    let salt = SaltString::generate(&mut OsRng);
    let hash = password::hash_password(&user_info.password, &salt)?;
    let new_user = NewUser {
        name: &user_info.name,
        password_hash: &hash,
        password_salt: salt.as_str(),
        email: user_info.email.as_deref(),
        rank: creation_rank,
        avatar_style: user_info.avatar_style.unwrap_or_default(),
    };
    let custom_avatar = user_info
        .avatar_token
        .map(|token| thumbnail::create_from_token(&token, ThumbnailType::Avatar))
        .transpose()?;

    let mut conn = db::get_connection()?;
    let user_id = conn.transaction(|conn| {
        let user_id = diesel::insert_into(user::table)
            .values(new_user)
            .returning(user::id)
            .get_result(conn)?;

        if let Some(avatar) = custom_avatar {
            api::verify_privilege(client, config::privileges().user_edit_any_avatar)?;

            let avatar_size = filesystem::save_custom_avatar(&user_info.name, avatar)?;
            diesel::update(user::table.find(user_id))
                .set(user::custom_avatar_size.eq(avatar_size as i64))
                .execute(conn)?;
        }

        Ok::<_, api::Error>(user_id)
    })?;
    conn.transaction(|conn| UserInfo::new_from_id(conn, user_id, &fields, Visibility::Full).map_err(api::Error::from))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct UserUpdate {
    version: DateTime,
    name: Option<String>,
    password: Option<String>,
    #[serde(default, deserialize_with = "api::deserialize_some")]
    email: Option<Option<String>>,
    rank: Option<UserRank>,
    avatar_style: Option<AvatarStyle>,
    avatar_token: Option<String>,
}

fn update_user(auth: AuthResult, username: String, query: ResourceQuery, update: UserUpdate) -> ApiResult<UserInfo> {
    let client = auth?;
    query.bump_login(client)?;

    let client_id = client.map(|user| user.id);
    let fields = create_field_table(query.fields())?;
    let username = percent_encoding::percent_decode_str(&username).decode_utf8()?;
    let custom_avatar = update
        .avatar_token
        .map(|token| thumbnail::create_from_token(&token, ThumbnailType::Avatar))
        .transpose()?;

    let mut conn = db::get_connection()?;
    let (user_id, visibility) = conn.transaction(|conn| {
        let (user_id, user_version): (i64, DateTime) = user::table
            .select((user::id, user::last_edit_time))
            .filter(user::name.eq(&username))
            .first(conn)?;
        api::verify_version(user_version, update.version)?;

        let editing_self = client_id == Some(user_id);
        let visibility = match editing_self {
            true => Visibility::Full,
            false => Visibility::PublicOnly,
        };

        if let Some(new_name) = update.name.as_deref() {
            let required_rank = match editing_self {
                true => config::privileges().user_edit_self_name,
                false => config::privileges().user_edit_any_name,
            };
            api::verify_privilege(client, required_rank)?;
            api::verify_matches_regex(new_name, RegexType::Username)?;

            // Update first to see if new name clashes with any existing names
            diesel::update(user::table.find(user_id))
                .set(user::name.eq(new_name))
                .execute(conn)?;

            let old_custom_avatar_path = hash::custom_avatar_path(&username);
            if old_custom_avatar_path.try_exists()? {
                let new_custom_avatar_path = hash::custom_avatar_path(new_name);
                std::fs::rename(old_custom_avatar_path, new_custom_avatar_path)?;
            }
        }
        if let Some(password) = update.password {
            let required_rank = match editing_self {
                true => config::privileges().user_edit_self_pass,
                false => config::privileges().user_edit_any_pass,
            };
            api::verify_privilege(client, required_rank)?;
            api::verify_matches_regex(&password, RegexType::Password)?;

            let salt = SaltString::generate(&mut OsRng);
            let hash = password::hash_password(&password, &salt)?;
            diesel::update(user::table.find(user_id))
                .set((user::password_salt.eq(salt.as_str()), user::password_hash.eq(hash)))
                .execute(conn)?;
        }
        if let Some(email) = update.email {
            let required_rank = match editing_self {
                true => config::privileges().user_edit_self_email,
                false => config::privileges().user_edit_any_email,
            };
            api::verify_privilege(client, required_rank)?;
            api::verify_valid_email(email.as_deref())?;

            diesel::update(user::table.find(user_id))
                .set(user::email.eq(email))
                .execute(conn)?;
        }
        if let Some(rank) = update.rank {
            let required_rank = match editing_self {
                true => config::privileges().user_edit_self_rank,
                false => config::privileges().user_edit_any_rank,
            };
            api::verify_privilege(client, required_rank)?;
            if rank > config::default_rank() {
                api::verify_privilege(client, rank)?;
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
            api::verify_privilege(client, required_rank)?;

            diesel::update(user::table.find(user_id))
                .set(user::avatar_style.eq(avatar_style))
                .execute(conn)?;
        }
        if let Some(avatar) = custom_avatar {
            let required_rank = match editing_self {
                true => config::privileges().user_edit_self_avatar,
                false => config::privileges().user_edit_any_avatar,
            };
            api::verify_privilege(client, required_rank)?;

            filesystem::delete_custom_avatar(&username)?;

            let name = update.name.as_deref().unwrap_or(&username);
            let avatar_size = filesystem::save_custom_avatar(name, avatar)?;
            diesel::update(user::table.find(user_id))
                .set(user::custom_avatar_size.eq(avatar_size as i64))
                .execute(conn)?;
        }

        // Update last_edit_time
        diesel::update(user::table.find(user_id))
            .set(user::last_edit_time.eq(DateTime::now()))
            .execute(conn)?;
        Ok::<_, api::Error>((user_id, visibility))
    })?;
    conn.transaction(|conn| UserInfo::new_from_id(conn, user_id, &fields, visibility).map_err(api::Error::from))
}

fn delete_user(auth: AuthResult, username: String, client_version: DeleteRequest) -> ApiResult<()> {
    let client = auth?;
    let client_id = client.map(|user| user.id);
    let username = percent_encoding::percent_decode_str(&username).decode_utf8()?;

    db::get_connection()?.transaction(|conn| {
        let (user_id, user_version): (i64, DateTime) = user::table
            .select((user::id, user::last_edit_time))
            .filter(user::name.eq(username))
            .first(conn)?;
        api::verify_version(user_version, *client_version)?;

        let deleting_self = client_id == Some(user_id);
        let required_rank = match deleting_self {
            true => config::privileges().user_delete_self,
            false => config::privileges().user_delete_any,
        };
        api::verify_privilege(client, required_rank)?;

        diesel::delete(user::table.find(user_id)).execute(conn)?;
        Ok(())
    })
}

#[cfg(test)]
mod test {
    use crate::api::ApiResult;
    use crate::model::user::User;
    use crate::schema::{database_statistics, user};
    use crate::test::*;
    use crate::time::DateTime;
    use diesel::dsl::exists;
    use diesel::prelude::*;
    use serial_test::{parallel, serial};

    // Exclude fields that involve creation_time or last_edit_time
    const FIELDS: &str = "&fields=name,email,rank,avatarStyle,avatarUrl,commentCount,uploadedPostCount,likedPostCount,dislikedPostCount,favoritePostCount";

    #[tokio::test]
    #[parallel]
    async fn list() -> ApiResult<()> {
        const QUERY: &str = "GET /users/?query";
        const SORT: &str = "-sort:name&limit=40";
        verify_query(&format!("{QUERY}={SORT}{FIELDS}"), "user/list.json").await?;
        verify_query(&format!("{QUERY}=name:*user* {SORT}{FIELDS}"), "user/list_has_user_in_name.json").await
    }

    #[tokio::test]
    #[parallel]
    async fn get() -> ApiResult<()> {
        const NAME: &str = "regular_user";
        let get_last_edit_time = |conn: &mut PgConnection| -> QueryResult<DateTime> {
            user::table
                .select(user::last_edit_time)
                .filter(user::name.eq(NAME))
                .first(conn)
        };

        let mut conn = get_connection()?;
        let last_edit_time = get_last_edit_time(&mut conn)?;

        verify_query(&format!("GET /user/{NAME}/?{FIELDS}"), "user/get.json").await?;

        let new_last_edit_time = get_last_edit_time(&mut conn)?;
        assert_eq!(new_last_edit_time, last_edit_time);
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn create() -> ApiResult<()> {
        let get_user_count = |conn: &mut PgConnection| -> QueryResult<i64> {
            database_statistics::table
                .select(database_statistics::user_count)
                .first(conn)
        };

        let mut conn = get_connection()?;
        let user_count = get_user_count(&mut conn)?;

        verify_query(&format!("POST /users/?{FIELDS}"), "user/create.json").await?;

        let (user_id, name): (i64, String) = user::table
            .select((user::id, user::name))
            .order_by(user::id.desc())
            .first(&mut conn)?;

        let new_user_count = get_user_count(&mut conn)?;
        assert_eq!(new_user_count, user_count + 1);

        verify_query(&format!("DELETE /user/{name}"), "delete.json").await?;

        let new_user_count = get_user_count(&mut conn)?;
        let has_user: bool = diesel::select(exists(user::table.find(user_id))).get_result(&mut conn)?;
        assert_eq!(new_user_count, user_count);
        assert!(!has_user);
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn update() -> ApiResult<()> {
        const NAME: &str = "restricted_user";

        let mut conn = get_connection()?;
        let user_id: i64 = user::table
            .select(user::id)
            .filter(user::name.eq(NAME))
            .first(&mut conn)?;
        let user: User = user::table.find(user_id).first(&mut conn)?;

        verify_query(&format!("PUT /user/{NAME}/?{FIELDS}"), "user/update.json").await?;

        let new_user: User = user::table.find(user_id).first(&mut conn)?;
        assert_eq!(new_user.id, user.id);
        assert_ne!(new_user.name, user.name);
        assert_eq!(new_user.password_hash, user.password_hash);
        assert_eq!(new_user.password_salt, user.password_salt);
        assert_ne!(new_user.email, user.email);
        assert_ne!(new_user.rank, user.rank);
        assert_ne!(new_user.avatar_style, user.avatar_style);
        assert_eq!(new_user.creation_time, user.creation_time);
        assert_eq!(new_user.last_login_time, user.last_login_time);
        assert!(new_user.last_edit_time > user.last_edit_time);

        let new_name = &new_user.name;
        verify_query(&format!("PUT /user/{new_name}/?{FIELDS}"), "user/update_restore.json").await?;

        let new_user: User = user::table.find(user_id).first(&mut conn)?;
        assert_eq!(new_user.id, user.id);
        assert_eq!(new_user.name, user.name);
        assert_eq!(new_user.password_hash, user.password_hash);
        assert_eq!(new_user.password_salt, user.password_salt);
        assert_eq!(new_user.email, user.email);
        assert_eq!(new_user.rank, user.rank);
        assert_eq!(new_user.avatar_style, user.avatar_style);
        assert_eq!(new_user.creation_time, user.creation_time);
        assert_eq!(new_user.last_login_time, user.last_login_time);
        assert!(new_user.last_edit_time > user.last_edit_time);
        Ok(())
    }
}
