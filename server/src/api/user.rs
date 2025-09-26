use crate::api::{ApiResult, DeleteBody, PageParams, PagedResponse, ResourceParams};
use crate::auth::Client;
use crate::auth::password;
use crate::config::RegexType;
use crate::content::thumbnail::ThumbnailType;
use crate::content::upload::{MAX_UPLOAD_SIZE, PartName};
use crate::content::{Content, FileContents, JsonOrMultipart, hash, upload};
use crate::model::enums::{AvatarStyle, ResourceType, UserRank};
use crate::model::user::{NewUser, User};
use crate::resource::user::{UserInfo, Visibility};
use crate::schema::user;
use crate::search::user::QueryBuilder;
use crate::string::SmallString;
use crate::time::DateTime;
use crate::{api, config, db, filesystem, resource, update};
use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use axum::extract::{DefaultBodyLimit, Extension, Path, Query};
use axum::{Json, Router, routing};
use diesel::prelude::*;
use serde::Deserialize;
use url::Url;

pub fn routes() -> Router {
    Router::new()
        .route("/users", routing::get(list).post(create_handler))
        .route(
            "/user/{name}",
            routing::get(get)
                .put(update_handler)
                .route_layer(DefaultBodyLimit::max(MAX_UPLOAD_SIZE))
                .delete(delete),
        )
        .layer(DefaultBodyLimit::max(MAX_UPLOAD_SIZE))
}

const MAX_USERS_PER_PAGE: i64 = 1000;

async fn list(
    Extension(client): Extension<Client>,
    Query(params): Query<PageParams>,
) -> ApiResult<Json<PagedResponse<UserInfo>>> {
    api::verify_privilege(client, config::privileges().user_list)?;

    let offset = params.offset.unwrap_or(0);
    let limit = std::cmp::min(params.limit.get(), MAX_USERS_PER_PAGE);
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    db::get_connection()?.transaction(|conn| {
        let mut query_builder = QueryBuilder::new(client, params.criteria())?;
        query_builder.set_offset_and_limit(offset, limit);

        let (total, selected_users) = query_builder.list(conn)?;
        Ok(Json(PagedResponse {
            query: params.into_query(),
            offset,
            limit,
            total,
            results: UserInfo::new_batch_from_ids(conn, &selected_users, &fields, Visibility::PublicOnly)?,
        }))
    })
}

async fn get(
    Extension(client): Extension<Client>,
    Path(username): Path<String>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<UserInfo>> {
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
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
        UserInfo::new_from_id(conn, user_id, &fields, visibility)
            .map(Json)
            .map_err(api::Error::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct CreateBody {
    name: SmallString,
    password: SmallString,
    email: Option<SmallString>,
    rank: Option<UserRank>,
    avatar_style: Option<AvatarStyle>,
    #[serde(skip_deserializing)]
    avatar: Option<FileContents>,
    avatar_token: Option<String>,
    avatar_url: Option<Url>,
}

async fn create(client: Client, params: ResourceParams, body: CreateBody) -> ApiResult<Json<UserInfo>> {
    let creation_rank = body.rank.unwrap_or(config::default_rank());
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

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    api::verify_matches_regex(&body.name, RegexType::Username)?;
    api::verify_matches_regex(&body.password, RegexType::Password)?;
    api::verify_valid_email(body.email.as_deref())?;

    let salt = SaltString::generate(&mut OsRng);
    let hash = password::hash_password(&body.password, &salt)?;
    let new_user = NewUser {
        name: &body.name,
        password_hash: &hash,
        password_salt: salt.as_str(),
        email: body.email.as_deref(),
        rank: creation_rank,
        avatar_style: body.avatar_style.unwrap_or_default(),
    };

    let custom_avatar = match Content::new(body.avatar, body.avatar_token, body.avatar_url) {
        Some(content) => Some(content.thumbnail(ThumbnailType::Avatar).await?),
        None => None,
    };

    let mut conn = db::get_connection()?;
    let user = conn.transaction(|conn| {
        let user: User = new_user.insert_into(user::table).get_result(conn)?;

        if let Some(avatar) = custom_avatar {
            let required_rank = match creating_self {
                true => config::privileges().user_edit_self_avatar,
                false => config::privileges().user_edit_any_avatar,
            };
            api::verify_privilege(client, required_rank)?;

            update::user::avatar(conn, user.id, &body.name, &avatar)?;
        }

        Ok::<_, api::Error>(user)
    })?;
    conn.transaction(|conn| UserInfo::new(conn, user, &fields, Visibility::Full))
        .map(Json)
        .map_err(api::Error::from)
}

async fn create_handler(
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
    body: JsonOrMultipart<CreateBody>,
) -> ApiResult<Json<UserInfo>> {
    match body {
        JsonOrMultipart::Json(payload) => create(client, params, payload).await,
        JsonOrMultipart::Multipart(payload) => {
            let decoded_body = upload::extract(payload, [PartName::Avatar]).await?;
            let metadata = decoded_body.metadata.ok_or(api::Error::MissingMetadata)?;
            let mut new_user: CreateBody = serde_json::from_slice(&metadata)?;
            if let [Some(avatar)] = decoded_body.files {
                new_user.avatar = Some(avatar);
                create(client, params, new_user).await
            } else {
                Err(api::Error::MissingFormData)
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct UpdateBody {
    version: DateTime,
    name: Option<SmallString>,
    password: Option<SmallString>,
    #[serde(default, deserialize_with = "api::deserialize_some")]
    email: Option<Option<SmallString>>,
    rank: Option<UserRank>,
    avatar_style: Option<AvatarStyle>,
    #[serde(skip_deserializing)]
    avatar: Option<FileContents>,
    avatar_token: Option<String>,
    avatar_url: Option<Url>,
}

async fn update(
    client: Client,
    username: String,
    params: ResourceParams,
    body: UpdateBody,
) -> ApiResult<Json<UserInfo>> {
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    let custom_avatar = match Content::new(body.avatar, body.avatar_token, body.avatar_url) {
        Some(content) => Some(content.thumbnail(ThumbnailType::Avatar).await?),
        None => None,
    };

    let mut conn = db::get_connection()?;
    let (user_id, visibility) = conn.transaction(|conn| {
        let (user_id, user_version): (i64, DateTime) = user::table
            .select((user::id, user::last_edit_time))
            .filter(user::name.eq(&username))
            .first(conn)?;
        api::verify_version(user_version, body.version)?;

        let editing_self = client.id == Some(user_id);
        let visibility = match editing_self {
            true => Visibility::Full,
            false => Visibility::PublicOnly,
        };

        if let Some(password) = body.password {
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
        if let Some(email) = body.email {
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
        if let Some(rank) = body.rank {
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
        if let Some(avatar_style) = body.avatar_style {
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

            update::user::avatar(conn, user_id, &username, &avatar)?;
        }
        if let Some(new_name) = body.name.as_deref() {
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
                filesystem::move_file(&old_custom_avatar_path, &new_custom_avatar_path)?;
            }
        }
        update::user::last_edit_time(conn, user_id).map(|()| (user_id, visibility))
    })?;
    conn.transaction(|conn| UserInfo::new_from_id(conn, user_id, &fields, visibility))
        .map(Json)
        .map_err(api::Error::from)
}

async fn update_handler(
    Extension(client): Extension<Client>,
    Path(username): Path<String>,
    Query(params): Query<ResourceParams>,
    body: JsonOrMultipart<UpdateBody>,
) -> ApiResult<Json<UserInfo>> {
    match body {
        JsonOrMultipart::Json(payload) => update(client, username, params, payload).await,
        JsonOrMultipart::Multipart(payload) => {
            let decoded_body = upload::extract(payload, [PartName::Avatar]).await?;
            let metadata = decoded_body.metadata.ok_or(api::Error::MissingMetadata)?;
            let mut user_update: UpdateBody = serde_json::from_slice(&metadata)?;
            if let [Some(avatar)] = decoded_body.files {
                user_update.avatar = Some(avatar);
                update(client, username, params, user_update).await
            } else {
                Err(api::Error::MissingFormData)
            }
        }
    }
}

async fn delete(
    Extension(client): Extension<Client>,
    Path(username): Path<String>,
    Json(client_version): Json<DeleteBody>,
) -> ApiResult<Json<()>> {
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
        Ok(Json(()))
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

        verify_query(&format!("DELETE /user/{name}"), "user/delete.json").await?;

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
