use crate::api::{ApiResult, AuthResult};
use crate::model::user::{NewUserToken, User, UserToken};
use crate::resource::user::MicroUser;
use crate::resource::user_token::UserTokenInfo;
use crate::schema::user_token;
use crate::util::DateTime;
use crate::{api, config};
use diesel::prelude::*;
use serde::Deserialize;
use uuid::Uuid;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let post_user_token = warp::post()
        .and(warp::path!("user-token" / String))
        .and(api::auth())
        .and(warp::body::json())
        .map(create_user_token)
        .map(api::Reply::from);
    let delete_user_token = warp::delete()
        .and(warp::path!("user-token" / String / Uuid))
        .and(api::auth())
        .map(delete_user_token)
        .map(api::Reply::from);

    post_user_token.or(delete_user_token)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PostUserTokenInfo {
    enabled: bool,
    note: Option<String>,
    expiration_time: Option<DateTime>,
}

fn create_user_token(username: String, auth: AuthResult, token_info: PostUserTokenInfo) -> ApiResult<UserTokenInfo> {
    let mut conn = crate::establish_connection()?;
    let user = User::from_name(&mut conn, &username)?;

    let client = auth?;
    let client_id = client.as_ref().map(|user| user.id);
    let required_rank = match client_id == Some(user.id) {
        true => config::privileges().user_token_create_self,
        false => config::privileges().user_token_create_any,
    };
    api::verify_privilege(client.as_ref(), required_rank)?;

    // Delete previous token, if it exists
    diesel::delete(user_token::table.find(user.id)).execute(&mut conn)?;

    let new_user_token = NewUserToken {
        user_id: user.id,
        token: Uuid::new_v4(),
        note: token_info.note.as_deref(),
        enabled: token_info.enabled,
        expiration_time: token_info.expiration_time,
    };
    let user_token: UserToken = diesel::insert_into(user_token::table)
        .values(&new_user_token)
        .returning(UserToken::as_returning())
        .get_result(&mut conn)?;
    Ok(UserTokenInfo::new(MicroUser::new(user), user_token))
}

fn delete_user_token(username: String, token: Uuid, auth: AuthResult) -> ApiResult<()> {
    let mut conn = crate::establish_connection()?;
    let user = User::from_name(&mut conn, &username)?;

    let client = auth?;
    let client_id = client.as_ref().map(|user| user.id);
    let required_rank = match client_id == Some(user.id) {
        true => config::privileges().user_token_delete_self,
        false => config::privileges().user_token_delete_any,
    };
    api::verify_privilege(client.as_ref(), required_rank)?;

    let user_token = UserToken::belonging_to(&user)
        .select(UserToken::as_select())
        .filter(user_token::token.eq(token))
        .first(&mut conn)?;
    user_token.delete(&mut conn).map_err(api::Error::from)
}
