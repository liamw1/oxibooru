use crate::api::error::{ApiError, ApiResult};
use crate::api::extract::{Json, Path, Query};
use crate::api::{ResourceParams, UnpagedResponse};
use crate::app::AppState;
use crate::auth::Client;
use crate::model::enums::AvatarStyle;
use crate::model::user::{NewUserToken, UserToken};
use crate::resource::user::MicroUser;
use crate::resource::user_token::UserTokenInfo;
use crate::schema::{user, user_token};
use crate::string::{LargeString, SmallString};
use crate::time::DateTime;
use crate::{api, resource};
use axum::extract::{Extension, State};
use axum::{Router, routing};
use diesel::{BoolExpressionMethods, Connection, ExpressionMethods, Insertable, QueryDsl, RunQueryDsl, SaveChangesDsl};
use serde::Deserialize;
use uuid::Uuid;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/user-tokens/{username}", routing::get(list))
        .route("/user-token/{username}", routing::post(create))
        .route("/user-token/{username}/{token}", routing::put(update).delete(delete))
}

/// See [listing-user-tokens](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#listing-user-tokens)
async fn list(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(username): Path<String>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<UnpagedResponse<UserTokenInfo>>> {
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let (avatar_style, user_tokens) = state.get_connection()?.transaction(|conn| {
        let (user_id, avatar_style): (i64, AvatarStyle) = user::table
            .select((user::id, user::avatar_style))
            .filter(user::name.eq(&username))
            .first(conn)?;

        let required_rank = if client.id == Some(user_id) {
            state.config.privileges().user_token_list_self
        } else {
            state.config.privileges().user_token_list_any
        };
        api::verify_privilege(client, required_rank)?;

        user_token::table
            .filter(user_token::user_id.eq(user_id))
            .load(conn)
            .map(|tokens| (avatar_style, tokens))
            .map_err(ApiError::from)
    })?;

    let username = SmallString::new(username);
    let results = user_tokens
        .into_iter()
        .map(|user_token| {
            UserTokenInfo::new(MicroUser::new(&state.config, username.clone(), avatar_style), user_token, &fields)
        })
        .collect();
    Ok(Json(UnpagedResponse { results }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct CreateBody {
    enabled: bool,
    note: Option<String>,
    expiration_time: Option<DateTime>,
}

/// See [creating-user-token](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#creating-user-token)
async fn create(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(username): Path<String>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<CreateBody>,
) -> ApiResult<Json<UserTokenInfo>> {
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    let mut conn = state.get_connection()?;
    let (user_token, avatar_style) = conn.transaction(|conn| {
        let (user_id, avatar_style): (i64, AvatarStyle) = user::table
            .select((user::id, user::avatar_style))
            .filter(user::name.eq(&username))
            .first(conn)?;

        let required_rank = if client.id == Some(user_id) {
            state.config.privileges().user_token_create_self
        } else {
            state.config.privileges().user_token_create_any
        };
        api::verify_privilege(client, required_rank)?;

        // Delete any expired or disabled tokens owned by user
        let current_time = DateTime::now();
        diesel::delete(user_token::table)
            .filter(user_token::user_id.eq(user_id))
            .filter(
                user_token::enabled
                    .eq(false)
                    .or(user_token::expiration_time.lt(current_time)),
            )
            .execute(conn)?;

        let user_token = NewUserToken {
            id: Uuid::new_v4(),
            user_id,
            note: body.note.as_deref(),
            enabled: body.enabled,
            expiration_time: body.expiration_time,
        }
        .insert_into(user_token::table)
        .get_result(conn)?;
        Ok::<_, ApiError>((user_token, avatar_style))
    })?;
    Ok(Json(UserTokenInfo::new(
        MicroUser::new(&state.config, username.into(), avatar_style),
        user_token,
        &fields,
    )))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct UpdateBody {
    version: DateTime,
    enabled: Option<bool>,
    note: Option<LargeString>,
    #[serde(default, deserialize_with = "api::deserialize_some")]
    expiration_time: Option<Option<DateTime>>,
}

/// See [updating-user-token](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#updating-user-token)
async fn update(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path((username, token)): Path<(String, Uuid)>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<UpdateBody>,
) -> ApiResult<Json<UserTokenInfo>> {
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    let mut conn = state.get_connection()?;
    let (updated_user_token, avatar_style) = conn.transaction(|conn| {
        let (user_id, avatar_style): (i64, AvatarStyle) = user::table
            .select((user::id, user::avatar_style))
            .filter(user::name.eq(&username))
            .first(conn)?;

        let required_rank = if client.id == Some(user_id) {
            state.config.privileges().user_token_edit_self
        } else {
            state.config.privileges().user_token_edit_any
        };
        api::verify_privilege(client, required_rank)?;

        let mut user_token: UserToken = user_token::table.find(token).first(conn)?;
        api::verify_version(user_token.last_edit_time, body.version)?;

        if let Some(enabled) = body.enabled {
            user_token.enabled = enabled;
        }
        if let Some(note) = body.note {
            user_token.note = note;
        }
        if let Some(expiration_time) = body.expiration_time {
            user_token.expiration_time = expiration_time;
        }
        user_token.last_edit_time = DateTime::now();

        let updated_user_token: UserToken = user_token.save_changes(conn)?;
        Ok::<_, ApiError>((updated_user_token, avatar_style))
    })?;
    Ok(Json(UserTokenInfo::new(
        MicroUser::new(&state.config, username.into(), avatar_style),
        updated_user_token,
        &fields,
    )))
}

/// See [deleting-user-token](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#deleting-user-token)
async fn delete(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path((username, token)): Path<(String, Uuid)>,
) -> ApiResult<Json<()>> {
    state.get_connection()?.transaction(|conn| {
        let user_token_owner: i64 = user::table
            .inner_join(user_token::table)
            .select(user_token::user_id)
            .filter(user::name.eq(username))
            .filter(user_token::id.eq(token))
            .first(conn)?;

        let required_rank = if client.id == Some(user_token_owner) {
            state.config.privileges().user_token_delete_self
        } else {
            state.config.privileges().user_token_delete_any
        };
        api::verify_privilege(client, required_rank)?;

        diesel::delete(user_token::table.find(token)).execute(conn)?;
        Ok(Json(()))
    })
}

#[cfg(test)]
mod test {
    use crate::api::error::ApiResult;
    use crate::model::user::UserToken;
    use crate::schema::{user, user_token};
    use crate::test::*;
    use diesel::dsl::exists;
    use diesel::{ExpressionMethods, PgConnection, QueryDsl, QueryResult, RunQueryDsl, SelectableHelper};
    use serial_test::{parallel, serial};
    use uuid::Uuid;

    // Exclude fields that involve token, creation_time, last_edit_time, or last_usage_time
    const FIELDS: &str = "&fields=user,note,enabled,expirationTime";

    #[tokio::test]
    #[parallel]
    async fn list() -> ApiResult<()> {
        const USER: &str = "administrator";
        verify_query(&format!("GET /user-tokens/{USER}/?{FIELDS}"), "user_token/list").await
    }

    #[tokio::test]
    #[serial]
    async fn create() -> ApiResult<()> {
        const USER: &str = "restricted_user";
        verify_query(&format!("POST /user-token/{USER}/?{FIELDS}"), "user_token/create").await?;

        let mut conn = get_connection()?;
        let token: Uuid = user_token::table
            .select(user_token::id)
            .order_by(user_token::creation_time.desc())
            .first(&mut conn)?;

        verify_query(&format!("DELETE /user-token/{USER}/{token}"), "user_token/delete").await?;

        let has_token: bool = diesel::select(exists(user_token::table.find(token))).get_result(&mut conn)?;
        assert!(!has_token);
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn update() -> ApiResult<()> {
        const USER: &str = "administrator";
        let get_user_token = |conn: &mut PgConnection| -> QueryResult<UserToken> {
            user::table
                .inner_join(user_token::table)
                .select(UserToken::as_select())
                .filter(user::name.eq(USER))
                .first(conn)
        };

        let mut conn = get_connection()?;
        let user_token = get_user_token(&mut conn)?;

        verify_query(&format!("PUT /user-token/{USER}/{TEST_TOKEN}/?{FIELDS}"), "user_token/update").await?;

        let new_user_token = get_user_token(&mut conn)?;
        assert_eq!(new_user_token.id, user_token.id);
        assert_eq!(new_user_token.user_id, user_token.user_id);
        assert_ne!(new_user_token.note, user_token.note);
        assert_ne!(new_user_token.enabled, user_token.enabled);
        assert_ne!(new_user_token.expiration_time, user_token.expiration_time);
        assert!(new_user_token.last_edit_time > user_token.last_edit_time);
        assert_eq!(new_user_token.last_usage_time, user_token.last_usage_time);

        verify_query(&format!("PUT /user-token/{USER}/{TEST_TOKEN}/?{FIELDS}"), "user_token/update_restore").await?;

        let new_user_token = get_user_token(&mut conn)?;
        assert_eq!(new_user_token.id, user_token.id);
        assert_eq!(new_user_token.user_id, user_token.user_id);
        assert_eq!(new_user_token.note, user_token.note);
        assert_eq!(new_user_token.enabled, user_token.enabled);
        assert_eq!(new_user_token.expiration_time, user_token.expiration_time);
        assert!(new_user_token.last_edit_time > user_token.last_edit_time);
        assert_eq!(new_user_token.last_usage_time, user_token.last_usage_time);
        Ok(())
    }
}
