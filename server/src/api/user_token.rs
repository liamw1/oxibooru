use crate::api::{ApiResult, AuthResult, UnpagedResponse};
use crate::model::enums::AvatarStyle;
use crate::model::user::{NewUserToken, UserToken};
use crate::resource::user::MicroUser;
use crate::resource::user_token::UserTokenInfo;
use crate::schema::{user, user_token};
use crate::time::DateTime;
use crate::{api, config, db};
use diesel::prelude::*;
use serde::Deserialize;
use uuid::Uuid;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list_user_tokens = warp::get()
        .and(warp::path!("user-tokens" / String))
        .and(api::auth())
        .map(list_user_tokens)
        .map(api::Reply::from);
    let create_user_token = warp::post()
        .and(warp::path!("user-token" / String))
        .and(api::auth())
        .and(warp::body::json())
        .map(create_user_token)
        .map(api::Reply::from);
    let update_user_token = warp::put()
        .and(warp::path!("user-token" / String / Uuid))
        .and(api::auth())
        .and(warp::body::json())
        .map(update_user_token)
        .map(api::Reply::from);
    let delete_user_token = warp::delete()
        .and(warp::path!("user-token" / String / Uuid))
        .and(api::auth())
        .map(delete_user_token)
        .map(api::Reply::from);

    list_user_tokens
        .or(create_user_token)
        .or(update_user_token)
        .or(delete_user_token)
}

fn list_user_tokens(username: String, auth: AuthResult) -> ApiResult<UnpagedResponse<UserTokenInfo>> {
    let client = auth?;
    let client_id = client.as_ref().map(|user| user.id);
    let username = percent_encoding::percent_decode_str(&username).decode_utf8()?;

    db::get_connection()?.transaction(|conn| {
        let (user_id, avatar_style): (i32, AvatarStyle) = user::table
            .select((user::id, user::avatar_style))
            .filter(user::name.eq(&username))
            .first(conn)?;

        let required_rank = match client_id == Some(user_id) {
            true => config::privileges().user_token_list_self,
            false => config::privileges().user_token_list_any,
        };
        api::verify_privilege(client.as_ref(), required_rank)?;

        let results = user_token::table
            .filter(user_token::user_id.eq(user_id))
            .load(conn)?
            .into_iter()
            .map(|user_token| UserTokenInfo::new(MicroUser::new(username.to_string(), avatar_style), user_token))
            .collect();
        Ok(UnpagedResponse { results })
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct PostUserTokenInfo {
    enabled: bool,
    note: Option<String>,
    expiration_time: Option<DateTime>,
}

fn create_user_token(username: String, auth: AuthResult, token_info: PostUserTokenInfo) -> ApiResult<UserTokenInfo> {
    let client = auth?;
    let client_id = client.as_ref().map(|user| user.id);
    let username = percent_encoding::percent_decode_str(&username).decode_utf8()?;

    db::get_connection()?.transaction(|conn| {
        let (user_id, avatar_style): (i32, AvatarStyle) = user::table
            .select((user::id, user::avatar_style))
            .filter(user::name.eq(&username))
            .first(conn)?;

        let required_rank = match client_id == Some(user_id) {
            true => config::privileges().user_token_create_self,
            false => config::privileges().user_token_create_any,
        };
        api::verify_privilege(client.as_ref(), required_rank)?;

        // Delete previous token, if it exists
        diesel::delete(user_token::table.find(user_id)).execute(conn)?;

        let new_user_token = NewUserToken {
            user_id,
            token: Uuid::new_v4(),
            note: token_info.note.as_deref(),
            enabled: token_info.enabled,
            expiration_time: token_info.expiration_time,
        };
        let user_token: UserToken = diesel::insert_into(user_token::table)
            .values(new_user_token)
            .returning(UserToken::as_returning())
            .get_result(conn)?;
        Ok(UserTokenInfo::new(MicroUser::new(username.to_string(), avatar_style), user_token))
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct UserTokenUpdate {
    version: DateTime,
    enabled: Option<bool>,
    note: Option<String>,
    expiration_time: Option<Option<DateTime>>,
}

fn update_user_token(
    username: String,
    token: Uuid,
    auth: AuthResult,
    update: UserTokenUpdate,
) -> ApiResult<UserTokenInfo> {
    let client = auth?;
    let client_id = client.as_ref().map(|user| user.id);
    let username = percent_encoding::percent_decode_str(&username).decode_utf8()?;

    db::get_connection()?.transaction(|conn| {
        let (user_id, avatar_style): (i32, AvatarStyle) = user::table
            .select((user::id, user::avatar_style))
            .filter(user::name.eq(&username))
            .first(conn)?;

        let required_rank = match client_id == Some(user_id) {
            true => config::privileges().user_token_edit_self,
            false => config::privileges().user_token_edit_any,
        };
        api::verify_privilege(client.as_ref(), required_rank)?;

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
        Ok(UserTokenInfo::new(MicroUser::new(username.to_string(), avatar_style), updated_user_token))
    })
}

fn delete_user_token(username: String, token: Uuid, auth: AuthResult) -> ApiResult<()> {
    let client = auth?;
    let client_id = client.as_ref().map(|user| user.id);
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
        api::verify_privilege(client.as_ref(), required_rank)?;

        diesel::delete(user_token::table.find(user_token_owner)).execute(conn)?;
        Ok(())
    })
}
