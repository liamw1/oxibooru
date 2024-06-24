use crate::api;
use crate::api::micro::MicroUser;
use crate::model::user::{NewUserToken, User, UserToken};
use crate::schema::user_token;
use crate::util::DateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use warp::hyper::body::Bytes;
use warp::Rejection;

pub async fn post_user(
    username: String,
    auth_result: api::AuthenticationResult,
    body: Bytes,
) -> Result<api::Reply, Rejection> {
    Ok(auth_result
        .and_then(|client| {
            api::parse_body(&body).and_then(|user_info| create_user_token(username, user_info, client.as_ref()))
        })
        .into())
}

pub async fn delete_user(
    username: String,
    token: Uuid,
    auth_result: api::AuthenticationResult,
) -> Result<api::Reply, Rejection> {
    Ok(auth_result
        .and_then(|client| remove_user_token(username, token, client.as_ref()))
        .into())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PostUserTokenInfo {
    enabled: bool,
    note: Option<String>,
    expiration_time: Option<DateTime>,
}

// TODO: Remove renames by changing references to these names in client
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UserTokenInfo {
    version: DateTime, // TODO: Remove last_edit_time as it fills the same role as version here
    user: MicroUser,
    token: Uuid,
    note: Option<String>,
    enabled: bool,
    expiration_time: Option<DateTime>,
    creation_time: DateTime,
    last_edit_time: DateTime,
    last_usage_time: DateTime,
}

impl UserTokenInfo {
    fn new(user: MicroUser, user_token: UserToken) -> Result<Self, api::Error> {
        Ok(UserTokenInfo {
            version: user_token.last_edit_time.clone().into(),
            user,
            token: user_token.token,
            note: user_token.note,
            enabled: user_token.enabled,
            expiration_time: user_token.expiration_time,
            creation_time: user_token.creation_time,
            last_edit_time: user_token.last_edit_time,
            last_usage_time: user_token.last_usage_time,
        })
    }
}

fn create_user_token(
    username: String,
    token_info: PostUserTokenInfo,
    client: Option<&User>,
) -> Result<UserTokenInfo, api::Error> {
    let mut conn = crate::establish_connection()?;
    let user = User::from_name(&mut conn, &username)?;

    let client_id = client.map(|user| user.id);
    let target = if client_id == Some(user.id) { "self" } else { "any" };
    let requested_action = "user_tokens:create:".to_owned() + target;
    api::validate_privilege(api::client_access_level(client), &requested_action)?;

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
    UserTokenInfo::new(MicroUser::new(user), user_token)
}

fn remove_user_token(username: String, token: Uuid, client: Option<&User>) -> Result<(), api::Error> {
    let mut conn = crate::establish_connection()?;
    let user = User::from_name(&mut conn, &username)?;

    let client_id = client.map(|user| user.id);
    let target = if client_id == Some(user.id) { "self" } else { "any" };
    let requested_action = "user_tokens:delete:".to_owned() + target;
    api::validate_privilege(api::client_access_level(client), &requested_action)?;

    let user_token = UserToken::belonging_to(&user)
        .select(UserToken::as_select())
        .filter(user_token::token.eq(token))
        .first(&mut conn)?;
    user_token.delete(&mut conn).map_err(api::Error::from)
}
