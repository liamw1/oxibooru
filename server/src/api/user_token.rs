use crate::api;
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

#[derive(Deserialize)]
struct NewUserTokenInfo {
    enabled: bool,
    note: Option<String>,
    #[serde(rename(deserialize = "expirationTime"))]
    expiration_time: Option<DateTime>,
}

// TODO: Remove renames by changing references to these names in client
#[derive(Serialize)]
struct UserTokenInfo {
    version: i32,
    user: api::MicroUser,
    token: Uuid,
    note: Option<String>,
    enabled: bool,
    #[serde(rename(serialize = "expirationTime"))]
    expiration_time: Option<DateTime>,
    #[serde(rename(serialize = "creationTime"))]
    creation_time: DateTime,
    #[serde(rename(serialize = "lastEditTime"))]
    last_edit_time: DateTime,
    #[serde(rename(serialize = "lastUsageTime"))]
    last_usage_time: DateTime,
}

impl UserTokenInfo {
    fn new(user: api::MicroUser, user_token: UserToken) -> Result<Self, api::Error> {
        Ok(UserTokenInfo {
            version: 0,
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
    token_info: NewUserTokenInfo,
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
    UserTokenInfo::new(api::MicroUser::new(user), user_token)
}
