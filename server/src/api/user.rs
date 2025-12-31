use crate::api::doc::USER_TAG;
use crate::api::error::{ApiError, ApiResult};
use crate::api::extract::{Json, JsonOrMultipart, Path, Query};
use crate::api::{DeleteBody, PageParams, PagedResponse, ResourceParams, error};
use crate::app::AppState;
use crate::auth::Client;
use crate::auth::password;
use crate::config::RegexType;
use crate::content::thumbnail::ThumbnailType;
use crate::content::upload::{MAX_UPLOAD_SIZE, PartName};
use crate::content::{Content, FileContents, upload};
use crate::model::enums::{AvatarStyle, ResourceProperty, ResourceType, UserRank};
use crate::model::user::{NewUser, User};
use crate::resource::user::{UserInfo, Visibility};
use crate::schema::user;
use crate::search::Builder;
use crate::search::user::QueryBuilder;
use crate::string::SmallString;
use crate::time::DateTime;
use crate::{api, filesystem, resource, update};
use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use axum::extract::{DefaultBodyLimit, Extension, State};
use diesel::dsl::exists;
use diesel::{Connection, ExpressionMethods, Insertable, OptionalExtension, QueryDsl, RunQueryDsl};
use serde::Deserialize;
use url::Url;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(get, update, delete))
        .route_layer(DefaultBodyLimit::max(MAX_UPLOAD_SIZE))
}

const MAX_USERS_PER_PAGE: i64 = 1000;

#[allow(dead_code)]
#[derive(ToSchema)]
struct Multipart<T> {
    /// JSON metadata (same structure as JSON request body).
    metadata: T,
    /// Avatar file.
    #[schema(format = Binary)]
    avatar: Option<String>,
}

/// Searches for users.
///
/// **Anonymous tokens**
///
/// Same as `name` token.
///
/// **Named tokens**
///
/// | Key                                                              | Description                                     |
/// | ---------------------------------------------------------------- | ----------------------------------------------- |
/// | `name`                                                           | having given name (accepts wildcards)           |
/// | `creation-date`, `creation-time`                                 | registered at given date                        |
/// | `last-login-date`, `last-login-time`, `login-date`, `login-time` | whose most recent login date matches given date |
///
/// **Sort style tokens**
///
/// | Value                                                            | Description             |
/// | ---------------------------------------------------------------- | ----------------------- |
/// | `random`                                                         | as random as it can get |
/// | `name`                                                           | A to Z                  |
/// | `creation-date`, `creation-time`                                 | newest to oldest        |
/// | `last-login-date`, `last-login-time`, `login-date`, `login-time` | recently active first   |
///
/// **Special tokens**
///
/// None.
#[utoipa::path(
    get,
    path = "/users",
    tag = USER_TAG,
    params(ResourceParams, PageParams),
    responses(
        (status = 200, body = PagedResponse<UserInfo>),
        (status = 403, description = "Privileges are too low"),
    ),
)]
async fn list(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Query(resource): Query<ResourceParams>,
    Query(page): Query<PageParams>,
) -> ApiResult<Json<PagedResponse<UserInfo>>> {
    api::verify_privilege(client, state.config.privileges().user_list)?;

    let offset = page.offset.unwrap_or(0);
    let limit = std::cmp::min(page.limit.get(), MAX_USERS_PER_PAGE);
    let fields = resource::create_table(resource.fields()).map_err(Box::from)?;

    state.get_connection()?.transaction(|conn| {
        let mut query_builder = QueryBuilder::new(client, resource.criteria())?;
        query_builder.set_offset_and_limit(offset, limit);

        let (total, selected_users) = query_builder.list(conn)?;
        Ok(Json(PagedResponse {
            query: resource.query,
            offset,
            limit,
            total,
            results: UserInfo::new_batch_from_ids(
                conn,
                &state.config,
                &selected_users,
                &fields,
                Visibility::PublicOnly,
            )?,
        }))
    })
}

/// Retrieves information about an existing user.
#[utoipa::path(
    get,
    path = "/user/{name}",
    tag = USER_TAG,
    params(
        ("name" = String, Path, description = "Username"),
        ResourceParams,
    ),
    responses(
        (status = 200, body = UserInfo),
        (status = 403, description = "Privileges are too low"),
        (status = 404, description = "The user does not exist"),
    ),
)]
async fn get(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(username): Path<SmallString>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<UserInfo>> {
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    state.get_connection()?.transaction(|conn| {
        let user_id = user::table
            .select(user::id)
            .filter(user::name.eq(username))
            .first(conn)
            .optional()?
            .ok_or(ApiError::NotFound(ResourceType::User))?;

        let viewing_self = client.id == Some(user_id);
        if !viewing_self {
            api::verify_privilege(client, state.config.privileges().user_view)?;
        }

        let visibility = if viewing_self {
            Visibility::Full
        } else {
            Visibility::PublicOnly
        };
        UserInfo::new_from_id(conn, &state.config, user_id, &fields, visibility)
            .map(Json)
            .map_err(ApiError::from)
    })
}

async fn create_impl(
    state: AppState,
    client: Client,
    params: ResourceParams,
    body: UserCreateBody,
) -> ApiResult<Json<UserInfo>> {
    let creation_rank = body.rank.unwrap_or(state.config.default_rank());
    if creation_rank == UserRank::Anonymous {
        return Err(ApiError::InvalidUserRank);
    }

    let creating_self = client.id.is_none();
    let required_rank = if creating_self {
        state.config.privileges().user_create_self
    } else {
        state.config.privileges().user_create_any
    };
    api::verify_privilege(client, required_rank)?;
    if creation_rank > state.config.default_rank() {
        api::verify_privilege(client, creation_rank)?;
    }

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    api::verify_matches_regex(&state.config, &body.name, RegexType::Username)?;
    api::verify_matches_regex(&state.config, &body.password, RegexType::Password)?;
    api::verify_valid_email(body.email.as_deref())?;

    let salt = SaltString::generate(&mut OsRng);
    let hash = password::hash_password(&state.config, &body.password, &salt)?;
    let new_user = NewUser {
        name: &body.name,
        password_hash: &hash,
        password_salt: salt.as_str(),
        email: body.email.as_deref(),
        rank: creation_rank,
        avatar_style: body.avatar_style.unwrap_or_default(),
    };

    let custom_avatar = match Content::new(body.avatar, body.avatar_token, body.avatar_url) {
        Some(content) => Some(content.thumbnail(&state.config, ThumbnailType::Avatar).await?),
        None if new_user.avatar_style == AvatarStyle::Manual => {
            return Err(ApiError::MissingContent(ResourceType::User));
        }
        None => None,
    };

    let mut conn = state.get_connection()?;
    let user = conn.transaction(|conn| {
        let name_exists: bool =
            diesel::select(exists(user::table.select(user::id).filter(user::name.eq(&new_user.name)))).first(conn)?;
        if name_exists {
            return Err(ApiError::AlreadyExists(ResourceProperty::UserName))?;
        }

        let user: User = new_user
            .insert_into(user::table)
            .on_conflict(user::email)
            .do_nothing()
            .get_result(conn)
            .optional()?
            .ok_or(ApiError::AlreadyExists(ResourceProperty::UserEmail))?;

        if let Some(avatar) = custom_avatar {
            let required_rank = if creating_self {
                state.config.privileges().user_edit_self_avatar
            } else {
                state.config.privileges().user_edit_any_avatar
            };
            api::verify_privilege(client, required_rank)?;

            update::user::avatar(conn, &state.config, user.id, &body.name, &avatar)?;
        }

        Ok::<_, ApiError>(user)
    })?;
    conn.transaction(|conn| UserInfo::new(conn, &state.config, user, &fields, Visibility::Full))
        .map(Json)
        .map_err(ApiError::from)
}

/// Request body for creating a user.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct UserCreateBody {
    /// Username. Must match `user_name_regex` from server's configuration.
    name: SmallString,
    /// Password. Must match `password_regex` from server's configuration.
    password: SmallString,
    /// Email address.
    email: Option<SmallString>,
    /// User rank. Defaults to `default_rank` from server's configuration.
    rank: Option<UserRank>,
    /// Avatar style.
    avatar_style: Option<AvatarStyle>,
    #[serde(skip_deserializing)]
    avatar: Option<FileContents>,
    /// Token referencing previously uploaded avatar.
    avatar_token: Option<String>,
    /// URL to fetch avatar from.
    avatar_url: Option<Url>,
}

/// Creates a new user using specified parameters.
///
/// Names and passwords must match `user_name_regex` and `password_regex` from
/// server's configuration, respectively. Email address, rank and avatar fields
/// are optional. Avatar style can be either `gravatar` or `manual`. `manual`
/// avatar style requires client to pass also `avatar` file - see
/// [file uploads](#Upload) for details. If the rank is empty and the
/// user happens to be the first user ever created, become an administrator,
/// whereas subsequent users will be given the rank indicated by `default_rank`
/// in the server's configuration.
#[utoipa::path(
    post,
    path = "/users",
    tag = USER_TAG,
    params(ResourceParams),
    request_body(
        content(
            (UserCreateBody = "application/json"),
            (Multipart<UserCreateBody> = "multipart/form-data"),
        )
    ),
    responses(
        (status = 200, body = UserInfo),
        (status = 400, description = "User name is invalid"),
        (status = 400, description = "Password is invalid"),
        (status = 400, description = "Email is invalid"),
        (status = 400, description = "Rank is invalid"),
        (status = 400, description = "Avatar is missing for manual avatar style"),
        (status = 403, description = "Privileges are too low"),
        (status = 403, description = "Trying to set rank higher than own rank"),
        (status = 409, description = "A user with such name already exists"),
    ),
)]
async fn create(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
    body: JsonOrMultipart<UserCreateBody>,
) -> ApiResult<Json<UserInfo>> {
    match body {
        JsonOrMultipart::Json(payload) => create_impl(state, client, params, payload).await,
        JsonOrMultipart::Multipart(payload) => {
            let decoded_body = upload::extract(payload, [PartName::Avatar]).await?;
            let metadata = decoded_body.metadata.ok_or(ApiError::MissingMetadata)?;
            let mut new_user: UserCreateBody = serde_json::from_slice(&metadata)?;
            if let [Some(avatar)] = decoded_body.files {
                new_user.avatar = Some(avatar);
                create_impl(state, client, params, new_user).await
            } else {
                Err(ApiError::MissingFormData)
            }
        }
    }
}

async fn update_impl(
    state: AppState,
    client: Client,
    username: &str,
    params: ResourceParams,
    body: UserUpdateBody,
) -> ApiResult<Json<UserInfo>> {
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    let custom_avatar = match Content::new(body.avatar, body.avatar_token, body.avatar_url) {
        Some(content) => Some(content.thumbnail(&state.config, ThumbnailType::Avatar).await?),
        None if body.avatar_style == Some(AvatarStyle::Manual) => {
            return Err(ApiError::MissingContent(ResourceType::User));
        }
        None => None,
    };

    let mut conn = state.get_connection()?;
    let (user_id, visibility) = conn.transaction(|conn| {
        let (user_id, user_version): (i64, DateTime) = user::table
            .select((user::id, user::last_edit_time))
            .filter(user::name.eq(&username))
            .first(conn)
            .optional()?
            .ok_or(ApiError::NotFound(ResourceType::User))?;
        api::verify_version(user_version, body.version)?;

        let editing_self = client.id == Some(user_id);
        let visibility = if editing_self {
            Visibility::Full
        } else {
            Visibility::PublicOnly
        };

        if let Some(password) = body.password {
            let required_rank = if editing_self {
                state.config.privileges().user_edit_self_pass
            } else {
                state.config.privileges().user_edit_any_pass
            };
            api::verify_privilege(client, required_rank)?;
            api::verify_matches_regex(&state.config, &password, RegexType::Password)?;

            let salt = SaltString::generate(&mut OsRng);
            let hash = password::hash_password(&state.config, &password, &salt)?;
            diesel::update(user::table.find(user_id))
                .set((user::password_salt.eq(salt.as_str()), user::password_hash.eq(hash)))
                .execute(conn)?;
        }
        if let Some(email) = body.email {
            let required_rank = if editing_self {
                state.config.privileges().user_edit_self_email
            } else {
                state.config.privileges().user_edit_any_email
            };
            api::verify_privilege(client, required_rank)?;
            api::verify_valid_email(email.as_deref())?;

            let update_result = diesel::update(user::table.find(user_id))
                .set(user::email.eq(email))
                .execute(conn);
            error::map_unique_violation(update_result, ResourceProperty::UserEmail)?;
        }
        if let Some(rank) = body.rank {
            if rank == UserRank::Anonymous {
                return Err(ApiError::InvalidUserRank);
            }

            let required_rank = if editing_self {
                state.config.privileges().user_edit_self_rank
            } else {
                state.config.privileges().user_edit_any_rank
            };
            api::verify_privilege(client, required_rank)?;
            if rank > state.config.default_rank() {
                api::verify_privilege(client, rank)?;
            }

            diesel::update(user::table.find(user_id))
                .set(user::rank.eq(rank))
                .execute(conn)?;
        }
        if let Some(avatar_style) = body.avatar_style {
            let required_rank = if editing_self {
                state.config.privileges().user_edit_self_avatar
            } else {
                state.config.privileges().user_edit_any_avatar
            };
            api::verify_privilege(client, required_rank)?;

            diesel::update(user::table.find(user_id))
                .set(user::avatar_style.eq(avatar_style))
                .execute(conn)?;
        }
        if let Some(avatar) = custom_avatar {
            let required_rank = if editing_self {
                state.config.privileges().user_edit_self_avatar
            } else {
                state.config.privileges().user_edit_any_avatar
            };
            api::verify_privilege(client, required_rank)?;

            update::user::avatar(conn, &state.config, user_id, username, &avatar)?;
        }
        if let Some(new_name) = body.name.as_deref() {
            let required_rank = if editing_self {
                state.config.privileges().user_edit_self_name
            } else {
                state.config.privileges().user_edit_any_name
            };
            api::verify_privilege(client, required_rank)?;
            api::verify_matches_regex(&state.config, new_name, RegexType::Username)?;

            // Update first to see if new name clashes with any existing names
            let update_result = diesel::update(user::table.find(user_id))
                .set(user::name.eq(new_name))
                .execute(conn);
            error::map_unique_violation(update_result, ResourceProperty::UserName)?;

            let old_custom_avatar_path = state.config.custom_avatar_path(username);
            if old_custom_avatar_path.try_exists()? {
                let new_custom_avatar_path = state.config.custom_avatar_path(new_name);
                filesystem::move_file(&old_custom_avatar_path, &new_custom_avatar_path)?;
            }
        }
        update::user::last_edit_time(conn, user_id)
            .map(|()| (user_id, visibility))
            .map_err(ApiError::from)
    })?;
    conn.transaction(|conn| UserInfo::new_from_id(conn, &state.config, user_id, &fields, visibility))
        .map(Json)
        .map_err(ApiError::from)
}

/// Request body for updating a user.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct UserUpdateBody {
    /// Resource version. See [versioning](#Versioning).
    version: DateTime,
    /// New username. Must match `user_name_regex` from server's configuration.
    name: Option<SmallString>,
    /// New password. Must match `password_regex` from server's configuration.
    password: Option<SmallString>,
    /// Email address. Set to null to remove.
    #[serde(default, deserialize_with = "api::deserialize_some")]
    email: Option<Option<SmallString>>,
    /// User rank.
    rank: Option<UserRank>,
    /// Avatar style.
    avatar_style: Option<AvatarStyle>,
    #[serde(skip_deserializing)]
    avatar: Option<FileContents>,
    /// Token referencing previously uploaded avatar.
    avatar_token: Option<String>,
    /// URL to fetch avatar from.
    avatar_url: Option<Url>,
}

/// Updates an existing user using specified parameters.
///
/// Names and passwords must match `user_name_regex` and `password_regex` from
/// server's configuration, respectively. Avatar style can be either `gravatar`
/// or `manual`. `manual` avatar style requires client to pass also `avatar`
/// file - see [file uploads](#Upload) for details. All fields except
/// `version` are optional - update concerns only provided fields. To update
/// last login time, see [authentication](#Authentication).
#[utoipa::path(
    put,
    path = "/user/{name}",
    tag = USER_TAG,
    params(
        ("name" = String, Path, description = "Username"),
        ResourceParams,
    ),
    request_body(
        content(
            (UserUpdateBody = "application/json"),
            (Multipart<UserUpdateBody> = "multipart/form-data"),
        )
    ),
    responses(
        (status = 200, body = UserInfo),
        (status = 400, description = "User name is invalid"),
        (status = 400, description = "Password is invalid"),
        (status = 400, description = "Email is invalid"),
        (status = 400, description = "Rank is invalid"),
        (status = 400, description = "Avatar is missing for manual avatar style"),
        (status = 403, description = "Privileges are too low"),
        (status = 403, description = "Trying to set rank higher than own rank"),
        (status = 404, description = "The user does not exist"),
        (status = 409, description = "The version is outdated"),
        (status = 409, description = "A user with new name already exists"),
    ),
)]
async fn update(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(username): Path<SmallString>,
    Query(params): Query<ResourceParams>,
    body: JsonOrMultipart<UserUpdateBody>,
) -> ApiResult<Json<UserInfo>> {
    match body {
        JsonOrMultipart::Json(payload) => update_impl(state, client, &username, params, payload).await,
        JsonOrMultipart::Multipart(payload) => {
            let decoded_body = upload::extract(payload, [PartName::Avatar]).await?;
            let metadata = decoded_body.metadata.ok_or(ApiError::MissingMetadata)?;
            let mut user_update: UserUpdateBody = serde_json::from_slice(&metadata)?;
            if let [Some(avatar)] = decoded_body.files {
                user_update.avatar = Some(avatar);
                update_impl(state, client, &username, params, user_update).await
            } else {
                Err(ApiError::MissingFormData)
            }
        }
    }
}

/// Deletes existing user.
#[utoipa::path(
    delete,
    path = "/user/{name}",
    tag = USER_TAG,
    params(
        ("name" = String, Path, description = "Username"),
    ),
    request_body = DeleteBody,
    responses(
        (status = 200, body = Object),
        (status = 403, description = "Privileges are too low"),
        (status = 404, description = "The user does not exist"),
        (status = 409, description = "The version is outdated"),
    ),
)]
async fn delete(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(username): Path<SmallString>,
    Json(client_version): Json<DeleteBody>,
) -> ApiResult<Json<()>> {
    state.get_connection()?.transaction(|conn| {
        let (user_id, user_version): (i64, DateTime) = user::table
            .select((user::id, user::last_edit_time))
            .filter(user::name.eq(username))
            .first(conn)
            .optional()?
            .ok_or(ApiError::NotFound(ResourceType::User))?;
        api::verify_version(user_version, *client_version)?;

        let deleting_self = client.id == Some(user_id);
        let required_rank = if deleting_self {
            state.config.privileges().user_delete_self
        } else {
            state.config.privileges().user_delete_any
        };
        api::verify_privilege(client, required_rank)?;

        diesel::delete(user::table.find(user_id)).execute(conn)?;
        Ok(Json(()))
    })
}

#[cfg(test)]
mod test {
    use crate::api::error::ApiResult;
    use crate::model::enums::{ResourceType, UserRank};
    use crate::model::user::User;
    use crate::schema::{database_statistics, user, user_statistics};
    use crate::search::user::Token;
    use crate::test::*;
    use crate::time::DateTime;
    use diesel::dsl::exists;
    use diesel::{ExpressionMethods, PgConnection, QueryDsl, QueryResult, RunQueryDsl, SelectableHelper};
    use serial_test::{parallel, serial};
    use strum::IntoEnumIterator;

    // Exclude fields that involve creation_time or last_edit_time
    const FIELDS: &str = "&fields=name,email,rank,avatarStyle,avatarUrl,commentCount,uploadedPostCount,likedPostCount,dislikedPostCount,favoritePostCount";

    #[tokio::test]
    #[parallel]
    async fn list() -> ApiResult<()> {
        const QUERY: &str = "GET /users/?query";
        const PARAMS: &str = "-sort:name&limit=40&fields=name";
        verify_response(&format!("{QUERY}=-sort:name&limit=40{FIELDS}"), "user/list").await?;

        let filter_table = crate::search::user::filter_table();
        for token in Token::iter() {
            let filter = filter_table[token];
            let (sign, filter) = if filter.starts_with('-') {
                filter.split_at(1)
            } else {
                ("", filter)
            };
            let query = format!("{QUERY}={sign}{token}:{filter} {PARAMS}");
            let path = format!("user/list_{token}_filtered");
            verify_response(&query, &path).await?;

            let query = format!("{QUERY}=sort:{token} {PARAMS}");
            let path = format!("user/list_{token}_sorted");
            verify_response(&query, &path).await?;
        }
        Ok(())
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

        verify_response(&format!("GET /user/{NAME}/?{FIELDS}"), "user/get").await?;
        verify_response_with_user(UserRank::Regular, &format!("GET /user/{NAME}/?{FIELDS}"), "user/get_self").await?;

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

        verify_response(&format!("POST /users/?{FIELDS}"), "user/create").await?;

        let (user_id, name): (i64, String) = user::table
            .select((user::id, user::name))
            .order_by(user::id.desc())
            .first(&mut conn)?;

        let new_user_count = get_user_count(&mut conn)?;
        assert_eq!(new_user_count, user_count + 1);

        verify_response(&format!("DELETE /user/{name}"), "user/delete").await?;

        let new_user_count = get_user_count(&mut conn)?;
        let has_user: bool = diesel::select(exists(user::table.find(user_id))).first(&mut conn)?;
        assert_eq!(new_user_count, user_count);
        assert!(!has_user);
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn update() -> ApiResult<()> {
        const NAME: &str = "moderator";

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

        verify_response(&format!("PUT /user/{NAME}/?{FIELDS}"), "user/edit").await?;

        let (new_user, new_comment_count, new_favorite_count, new_upload_count) = get_user_info(&mut conn)?;
        assert_eq!(new_user.id, user.id);
        assert_ne!(new_user.name, user.name);
        assert_eq!(new_user.password_hash, user.password_hash);
        assert_eq!(new_user.password_salt, user.password_salt);
        assert_ne!(new_user.email, user.email);
        assert_ne!(new_user.rank, user.rank);
        assert_eq!(new_user.avatar_style, user.avatar_style);
        assert_eq!(new_user.creation_time, user.creation_time);
        assert_eq!(new_user.last_login_time, user.last_login_time);
        assert!(new_user.last_edit_time > user.last_edit_time);
        assert_eq!(new_comment_count, comment_count);
        assert_eq!(new_favorite_count, favorite_count);
        assert_eq!(new_upload_count, upload_count);

        let new_name = &new_user.name;
        verify_response(&format!("PUT /user/{new_name}/?{FIELDS}"), "user/edit_restore").await?;

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

    #[tokio::test]
    #[parallel]
    async fn error() -> ApiResult<()> {
        verify_response("GET /user/fake_user", "user/get_nonexistent").await?;
        verify_response("PUT /user/fake_user", "user/edit_nonexistent").await?;
        verify_response("DELETE /user/fake_user", "user/delete_nonexistent").await?;

        verify_response("POST /users", "user/create_anonymous").await?;
        verify_response("POST /users", "user/create_name_clash").await?;
        verify_response("POST /users", "user/create_email_clash").await?;
        verify_response("POST /users", "user/create_invalid_name").await?;
        verify_response("POST /users", "user/create_invalid_rank").await?;
        verify_response("POST /users", "user/create_invalid_email").await?;
        verify_response("POST /users", "user/create_invalid_password").await?;
        verify_response("POST /users", "user/create_missing_custom_avatar").await?;

        verify_response("PUT /user/regular_user", "user/edit_anonymous").await?;
        verify_response("PUT /user/regular_user", "user/edit_name_clash").await?;
        verify_response("PUT /user/regular_user", "user/edit_email_clash").await?;
        verify_response("PUT /user/regular_user", "user/edit_invalid_name").await?;
        verify_response("PUT /user/regular_user", "user/edit_invalid_rank").await?;
        verify_response("PUT /user/regular_user", "user/edit_invalid_email").await?;
        verify_response("PUT /user/regular_user", "user/edit_invalid_password").await?;
        verify_response("PUT /user/regular_user", "user/edit_missing_custom_avatar").await?;

        // User has permissions to edit/delete self, but not another
        verify_response_with_user(UserRank::Regular, "PUT /user/power_user", "user/edit_another").await?;
        verify_response_with_user(UserRank::Regular, "DELETE /user/power_user", "user/delete_another").await?;

        verify_response_with_user(UserRank::Regular, "POST /users", "user/create_higher_rank").await?;
        verify_response_with_user(UserRank::Regular, "PUT /user/restricted_user", "user/edit_higher_rank").await?;

        reset_sequence(ResourceType::User)?;
        Ok(())
    }
}
