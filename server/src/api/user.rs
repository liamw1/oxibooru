use crate::api::{ApiResult, AuthResult, DeleteRequest, PagedQuery, PagedResponse, ResourceQuery};
use crate::auth::password;
use crate::config::RegexType;
use crate::content::thumbnail::ThumbnailType;
use crate::content::upload::{PartName, MAX_UPLOAD_SIZE};
use crate::content::{hash, upload, Content, FileContents};
use crate::model::enums::{AvatarStyle, ResourceType, UserRank};
use crate::model::user::NewUser;
use crate::resource::user::{UserInfo, Visibility};
use crate::schema::{database_statistics, user};
use crate::time::DateTime;
use crate::{api, config, db, resource, search, update};
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use diesel::prelude::*;
use serde::Deserialize;
use url::Url;
use warp::filters::multipart::FormData;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list = warp::get()
        .and(api::auth())
        .and(warp::path!("users"))
        .and(warp::query())
        .map(list)
        .map(api::Reply::from);
    let get = warp::get()
        .and(api::auth())
        .and(warp::path!("user" / String))
        .and(api::resource_query())
        .map(get)
        .map(api::Reply::from);
    let create = warp::post()
        .and(api::auth())
        .and(warp::path!("users"))
        .and(api::resource_query())
        .and(warp::body::json())
        .then(create)
        .map(api::Reply::from);
    let create_multipart = warp::post()
        .and(api::auth())
        .and(warp::path!("users"))
        .and(api::resource_query())
        .and(warp::filters::multipart::form().max_length(MAX_UPLOAD_SIZE))
        .then(create_multipart)
        .map(api::Reply::from);
    let update = warp::put()
        .and(api::auth())
        .and(warp::path!("user" / String))
        .and(api::resource_query())
        .and(warp::body::json())
        .then(update)
        .map(api::Reply::from);
    let update_multipart = warp::put()
        .and(api::auth())
        .and(warp::path!("user" / String))
        .and(api::resource_query())
        .and(warp::filters::multipart::form().max_length(MAX_UPLOAD_SIZE))
        .then(update_multipart)
        .map(api::Reply::from);
    let delete = warp::delete()
        .and(api::auth())
        .and(warp::path!("user" / String))
        .and(warp::body::json())
        .map(delete)
        .map(api::Reply::from);

    list.or(get)
        .or(create)
        .or(create_multipart)
        .or(update)
        .or(update_multipart)
        .or(delete)
}

const MAX_USERS_PER_PAGE: i64 = 1000;

fn list(auth: AuthResult, query: PagedQuery) -> ApiResult<PagedResponse<UserInfo>> {
    let client = auth?;
    query.bump_login(client)?;
    api::verify_privilege(client, config::privileges().user_list)?;

    let offset = query.offset.unwrap_or(0);
    let limit = std::cmp::min(query.limit.get(), MAX_USERS_PER_PAGE);
    let fields = resource::create_table(query.fields()).map_err(Box::from)?;

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

fn get(auth: AuthResult, username: String, query: ResourceQuery) -> ApiResult<UserInfo> {
    let client = auth?;
    query.bump_login(client)?;

    let fields = resource::create_table(query.fields()).map_err(Box::from)?;
    let username = percent_encoding::percent_decode_str(&username).decode_utf8()?;
    db::get_connection()?.transaction(|conn| {
        let user_id = user::table
            .select(user::id)
            .filter(user::name.eq(username))
            .first(conn)
            .optional()?
            .ok_or(api::Error::NotFound(ResourceType::User))?;

        let viewing_self = client.id == Some(user_id);
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
    #[serde(skip_deserializing)]
    avatar: Option<FileContents>,
    avatar_token: Option<String>,
    avatar_url: Option<Url>,
}

async fn create(auth: AuthResult, query: ResourceQuery, user_info: NewUserInfo) -> ApiResult<UserInfo> {
    let client = auth?;
    query.bump_login(client)?;

    let creation_rank = user_info.rank.unwrap_or(config::default_rank());
    if creation_rank == UserRank::Anonymous {
        return Err(api::Error::InvalidUserRank);
    }

    let creating_self = client.id.is_none();
    let required_rank = match creating_self {
        true => config::privileges().user_create_self,
        false => config::privileges().user_create_any,
    };
    api::verify_privilege(client, required_rank)?;
    if creation_rank > config::default_rank() {
        api::verify_privilege(client, creation_rank)?;
    }

    let fields = resource::create_table(query.fields()).map_err(Box::from)?;
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

    let custom_avatar = match Content::new(user_info.avatar, user_info.avatar_token, user_info.avatar_url) {
        Some(content) => Some(content.thumbnail(ThumbnailType::Avatar).await?),
        None => None,
    };

    let mut conn = db::get_connection()?;
    let user_id = conn.transaction(|conn| {
        let user_id = diesel::insert_into(user::table)
            .values(new_user)
            .returning(user::id)
            .get_result(conn)?;

        if let Some(avatar) = custom_avatar {
            let required_rank = match creating_self {
                true => config::privileges().user_edit_self_avatar,
                false => config::privileges().user_edit_any_avatar,
            };
            api::verify_privilege(client, required_rank)?;

            update::user::avatar(conn, user_id, &user_info.name, avatar)?;
        }

        Ok::<_, api::Error>(user_id)
    })?;
    conn.transaction(|conn| UserInfo::new_from_id(conn, user_id, &fields, Visibility::Full).map_err(api::Error::from))
}

async fn create_multipart(auth: AuthResult, query: ResourceQuery, form_data: FormData) -> ApiResult<UserInfo> {
    let body = upload::extract_with_metadata(form_data, [PartName::Avatar]).await?;
    let metadata = body.metadata.ok_or(api::Error::MissingMetadata)?;
    let mut user_info: NewUserInfo = serde_json::from_slice(&metadata)?;
    if let [Some(avatar)] = body.files {
        user_info.avatar = Some(avatar);
        create(auth, query, user_info).await
    } else {
        Err(api::Error::MissingFormData)
    }
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
    #[serde(skip_deserializing)]
    avatar: Option<FileContents>,
    avatar_token: Option<String>,
    avatar_url: Option<Url>,
}

async fn update(auth: AuthResult, username: String, query: ResourceQuery, update: UserUpdate) -> ApiResult<UserInfo> {
    let client = auth?;
    query.bump_login(client)?;

    let fields = resource::create_table(query.fields()).map_err(Box::from)?;
    let username = percent_encoding::percent_decode_str(&username).decode_utf8()?;

    let custom_avatar = match Content::new(update.avatar, update.avatar_token, update.avatar_url) {
        Some(content) => Some(content.thumbnail(ThumbnailType::Avatar).await?),
        None => None,
    };

    let mut conn = db::get_connection()?;
    let (user_id, visibility) = conn.transaction(|conn| {
        let (user_id, user_version): (i64, DateTime) = user::table
            .select((user::id, user::last_edit_time))
            .filter(user::name.eq(&username))
            .first(conn)?;
        api::verify_version(user_version, update.version)?;

        let editing_self = client.id == Some(user_id);
        let visibility = match editing_self {
            true => Visibility::Full,
            false => Visibility::PublicOnly,
        };

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

            update::user::avatar(conn, user_id, &username, avatar)?;
        }
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
        update::user::last_edit_time(conn, user_id).map(|_| (user_id, visibility))
    })?;
    conn.transaction(|conn| UserInfo::new_from_id(conn, user_id, &fields, visibility).map_err(api::Error::from))
}

async fn update_multipart(
    auth: AuthResult,
    username: String,
    query: ResourceQuery,
    form_data: FormData,
) -> ApiResult<UserInfo> {
    let body = upload::extract_with_metadata(form_data, [PartName::Avatar]).await?;
    let metadata = body.metadata.ok_or(api::Error::MissingMetadata)?;
    let mut user_update: UserUpdate = serde_json::from_slice(&metadata)?;
    if let [Some(avatar)] = body.files {
        user_update.avatar = Some(avatar);
        update(auth, username, query, user_update).await
    } else {
        Err(api::Error::MissingFormData)
    }
}

fn delete(auth: AuthResult, username: String, client_version: DeleteRequest) -> ApiResult<()> {
    let client = auth?;
    let username = percent_encoding::percent_decode_str(&username).decode_utf8()?;
    db::get_connection()?.transaction(|conn| {
        let (user_id, user_version): (i64, DateTime) = user::table
            .select((user::id, user::last_edit_time))
            .filter(user::name.eq(username))
            .first(conn)?;
        api::verify_version(user_version, *client_version)?;

        let deleting_self = client.id == Some(user_id);
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
    use crate::schema::{database_statistics, user, user_statistics};
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

        let get_user_info = |conn: &mut PgConnection| -> QueryResult<(User, i64, i64, i64)> {
            user::table
                .find(user_id)
                .inner_join(user_statistics::table)
                .select((
                    User::as_select(),
                    user_statistics::comment_count,
                    user_statistics::favorite_count,
                    user_statistics::upload_count,
                ))
                .first(conn)
        };

        let (user, comment_count, favorite_count, upload_count) = get_user_info(&mut conn)?;

        verify_query(&format!("PUT /user/{NAME}/?{FIELDS}"), "user/update.json").await?;

        let (new_user, new_comment_count, new_favorite_count, new_upload_count) = get_user_info(&mut conn)?;
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
        assert_eq!(new_comment_count, comment_count);
        assert_eq!(new_favorite_count, favorite_count);
        assert_eq!(new_upload_count, upload_count);

        let new_name = &new_user.name;
        verify_query(&format!("PUT /user/{new_name}/?{FIELDS}"), "user/update_restore.json").await?;

        let (new_user, new_comment_count, new_favorite_count, new_upload_count) = get_user_info(&mut conn)?;
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
        assert_eq!(new_comment_count, comment_count);
        assert_eq!(new_favorite_count, favorite_count);
        assert_eq!(new_upload_count, upload_count);
        Ok(())
    }
}
