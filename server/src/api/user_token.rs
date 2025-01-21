use crate::api::{ApiResult, AuthResult, ResourceQuery, UnpagedResponse};
use crate::model::enums::AvatarStyle;
use crate::model::user::{NewUserToken, UserToken};
use crate::resource::user::MicroUser;
use crate::resource::user_token::{FieldTable, UserTokenInfo};
use crate::schema::{user, user_token};
use crate::time::DateTime;
use crate::{api, config, db, resource};
use diesel::prelude::*;
use serde::Deserialize;
use uuid::Uuid;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list_user_tokens = warp::get()
        .and(api::auth())
        .and(warp::path!("user-tokens" / String))
        .and(api::resource_query())
        .map(list_user_tokens)
        .map(api::Reply::from);
    let create_user_token = warp::post()
        .and(api::auth())
        .and(warp::path!("user-token" / String))
        .and(api::resource_query())
        .and(warp::body::json())
        .map(create_user_token)
        .map(api::Reply::from);
    let update_user_token = warp::put()
        .and(api::auth())
        .and(warp::path!("user-token" / String / Uuid))
        .and(api::resource_query())
        .and(warp::body::json())
        .map(update_user_token)
        .map(api::Reply::from);
    let delete_user_token = warp::delete()
        .and(api::auth())
        .and(warp::path!("user-token" / String / Uuid))
        .map(delete_user_token)
        .map(api::Reply::from);

    list_user_tokens
        .or(create_user_token)
        .or(update_user_token)
        .or(delete_user_token)
}

fn create_field_table(fields: Option<&str>) -> Result<FieldTable<bool>, Box<dyn std::error::Error>> {
    fields
        .map(resource::user_token::Field::create_table)
        .transpose()
        .map(|opt_table| opt_table.unwrap_or(FieldTable::filled(true)))
        .map_err(Box::from)
}

fn list_user_tokens(
    auth: AuthResult,
    username: String,
    query: ResourceQuery,
) -> ApiResult<UnpagedResponse<UserTokenInfo>> {
    let client = auth?;
    let client_id = client.map(|user| user.id);
    let username = percent_encoding::percent_decode_str(&username).decode_utf8()?;
    let fields = create_field_table(query.fields())?;

    db::get_connection()?.transaction(|conn| {
        let (user_id, avatar_style): (i32, AvatarStyle) = user::table
            .select((user::id, user::avatar_style))
            .filter(user::name.eq(&username))
            .first(conn)?;

        let required_rank = match client_id == Some(user_id) {
            true => config::privileges().user_token_list_self,
            false => config::privileges().user_token_list_any,
        };
        api::verify_privilege(client, required_rank)?;

        let results = user_token::table
            .filter(user_token::user_id.eq(user_id))
            .load(conn)?
            .into_iter()
            .map(|user_token| {
                UserTokenInfo::new(MicroUser::new(username.to_string(), avatar_style), user_token, &fields)
            })
            .collect();
        Ok(UnpagedResponse { results })
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct NewUserTokenInfo {
    enabled: bool,
    note: Option<String>,
    expiration_time: Option<DateTime>,
}

fn create_user_token(
    auth: AuthResult,
    username: String,
    query: ResourceQuery,
    token_info: NewUserTokenInfo,
) -> ApiResult<UserTokenInfo> {
    let client = auth?;
    let client_id = client.map(|user| user.id);
    let username = percent_encoding::percent_decode_str(&username).decode_utf8()?;
    let fields = create_field_table(query.fields())?;

    let mut conn = db::get_connection()?;
    let (user_token, avatar_style) = conn.transaction(|conn| {
        let (user_id, avatar_style): (i32, AvatarStyle) = user::table
            .select((user::id, user::avatar_style))
            .filter(user::name.eq(&username))
            .first(conn)?;

        let required_rank = match client_id == Some(user_id) {
            true => config::privileges().user_token_create_self,
            false => config::privileges().user_token_create_any,
        };
        api::verify_privilege(client, required_rank)?;

        // Delete previous token, if it exists
        diesel::delete(user_token::table.find(user_id)).execute(conn)?;

        let new_user_token = NewUserToken {
            user_id,
            token: Uuid::new_v4(),
            note: token_info.note.as_deref(),
            enabled: token_info.enabled,
            expiration_time: token_info.expiration_time,
        };
        let user_token = diesel::insert_into(user_token::table)
            .values(new_user_token)
            .returning(UserToken::as_returning())
            .get_result(conn)?;
        Ok::<_, api::Error>((user_token, avatar_style))
    })?;
    Ok(UserTokenInfo::new(MicroUser::new(username.to_string(), avatar_style), user_token, &fields))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct UserTokenUpdate {
    version: DateTime,
    enabled: Option<bool>,
    note: Option<String>,
    #[serde(default, deserialize_with = "api::deserialize_some")]
    expiration_time: Option<Option<DateTime>>,
}

fn update_user_token(
    auth: AuthResult,
    username: String,
    token: Uuid,
    query: ResourceQuery,
    update: UserTokenUpdate,
) -> ApiResult<UserTokenInfo> {
    let client = auth?;
    let client_id = client.map(|user| user.id);
    let username = percent_encoding::percent_decode_str(&username).decode_utf8()?;
    let fields = create_field_table(query.fields())?;

    let mut conn = db::get_connection()?;
    let (updated_user_token, avatar_style) = conn.transaction(|conn| {
        let (user_id, avatar_style): (i32, AvatarStyle) = user::table
            .select((user::id, user::avatar_style))
            .filter(user::name.eq(&username))
            .first(conn)?;

        let required_rank = match client_id == Some(user_id) {
            true => config::privileges().user_token_edit_self,
            false => config::privileges().user_token_edit_any,
        };
        api::verify_privilege(client, required_rank)?;

        let user_token_version = user_token::table
            .find(user_id)
            .select(user_token::last_edit_time)
            .first(conn)?;
        api::verify_version(user_token_version, update.version)?;

        if let Some(enabled) = update.enabled {
            diesel::update(user_token::table)
                .filter(user_token::user_id.eq(user_id))
                .filter(user_token::token.eq(token))
                .set(user_token::enabled.eq(enabled))
                .execute(conn)?;
        }
        if let Some(note) = update.note {
            diesel::update(user_token::table)
                .filter(user_token::user_id.eq(user_id))
                .filter(user_token::token.eq(token))
                .set(user_token::note.eq(note))
                .execute(conn)?;
        }
        if let Some(expiration_time) = update.expiration_time {
            diesel::update(user_token::table)
                .filter(user_token::user_id.eq(user_id))
                .filter(user_token::token.eq(token))
                .set(user_token::expiration_time.eq(expiration_time))
                .execute(conn)?;
        }

        let updated_user_token: UserToken = user_token::table.find(user_id).first(conn)?;
        Ok::<_, api::Error>((updated_user_token, avatar_style))
    })?;
    Ok(UserTokenInfo::new(MicroUser::new(username.to_string(), avatar_style), updated_user_token, &fields))
}

fn delete_user_token(auth: AuthResult, username: String, token: Uuid) -> ApiResult<()> {
    let client = auth?;
    let client_id = client.map(|user| user.id);
    let username = percent_encoding::percent_decode_str(&username).decode_utf8()?;

    db::get_connection()?.transaction(|conn| {
        let user_token_owner: i32 = user::table
            .inner_join(user_token::table)
            .select(user_token::user_id)
            .filter(user::name.eq(username))
            .filter(user_token::token.eq(token))
            .first(conn)?;

        let required_rank = match client_id == Some(user_token_owner) {
            true => config::privileges().user_token_delete_self,
            false => config::privileges().user_token_delete_any,
        };
        api::verify_privilege(client, required_rank)?;

        diesel::delete(user_token::table.find(user_token_owner)).execute(conn)?;
        Ok(())
    })
}

#[cfg(test)]
mod test {
    use crate::api::ApiResult;
    use crate::model::user::UserToken;
    use crate::schema::{user, user_token};
    use crate::test::*;
    use diesel::dsl::exists;
    use diesel::prelude::*;
    use serial_test::{parallel, serial};
    use uuid::Uuid;

    // Exclude fields that involve token, creation_time, last_edit_time, or last_usage_time
    const FIELDS: &str = "&fields=user,note,enabled,expirationTime";

    #[tokio::test]
    #[parallel]
    async fn list() -> ApiResult<()> {
        const USER: &str = "administrator";
        verify_query(&format!("GET /user-tokens/{USER}/?{FIELDS}"), "user_token/list.json").await
    }

    #[tokio::test]
    #[serial]
    async fn create() -> ApiResult<()> {
        const USER: &str = "restricted_user";
        verify_query(&format!("POST /user-token/{USER}/?{FIELDS}"), "user_token/create.json").await?;

        let mut conn = get_connection()?;
        let (user_id, token): (i32, Uuid) = user_token::table
            .select((user_token::user_id, user_token::token))
            .order_by(user_token::creation_time.desc())
            .first(&mut conn)?;

        verify_query(&format!("DELETE /user-token/{USER}/{token}"), "delete.json").await?;

        let has_token: bool = diesel::select(exists(user_token::table.find(user_id))).get_result(&mut conn)?;
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

        verify_query(&format!("PUT /user-token/{USER}/{TEST_TOKEN}/?{FIELDS}"), "user_token/update.json").await?;

        let new_user_token = get_user_token(&mut conn)?;
        assert_eq!(new_user_token.user_id, user_token.user_id);
        assert_eq!(new_user_token.token, user_token.token);
        assert_ne!(new_user_token.note, user_token.note);
        assert_ne!(new_user_token.enabled, user_token.enabled);
        assert_ne!(new_user_token.expiration_time, user_token.expiration_time);
        assert!(new_user_token.last_edit_time > user_token.last_edit_time);
        assert_eq!(new_user_token.last_usage_time, user_token.last_usage_time);

        verify_query(&format!("PUT /user-token/{USER}/{TEST_TOKEN}/?{FIELDS}"), "user_token/update_restore.json")
            .await?;

        let new_user_token = get_user_token(&mut conn)?;
        assert_eq!(new_user_token.user_id, user_token.user_id);
        assert_eq!(new_user_token.token, user_token.token);
        assert_eq!(new_user_token.note, user_token.note);
        assert_eq!(new_user_token.enabled, user_token.enabled);
        assert_eq!(new_user_token.expiration_time, user_token.expiration_time);
        assert!(new_user_token.last_edit_time > user_token.last_edit_time);
        assert_eq!(new_user_token.last_usage_time, user_token.last_usage_time);
        Ok(())
    }
}
