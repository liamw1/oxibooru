use crate::api;
use crate::model::user::{NewUserToken, User, UserToken};
use serde::{Deserialize, Serialize};
use time::serde::rfc3339;
use time::OffsetDateTime;
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
    note: String,
    #[serde(with = "rfc3339", rename(serialize = "expirationTime"))]
    expiration_time: OffsetDateTime,
}

// TODO: Remove renames by changing references to these names in client
#[derive(Serialize)]
struct UserTokenInfo {
    version: i32,
    user: String,
    token: String,
    note: String,
    enabled: bool,
    #[serde(with = "rfc3339", rename(serialize = "expirationTime"))]
    expiration_time: OffsetDateTime,
    #[serde(with = "rfc3339", rename(serialize = "creationTime"))]
    creation_time: OffsetDateTime,
    #[serde(with = "rfc3339", rename(serialize = "lastEditTime"))]
    last_edit_time: OffsetDateTime,
    #[serde(with = "rfc3339", rename(serialize = "lastUsageTime"))]
    last_usage_time: OffsetDateTime,
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

    Ok(UserTokenInfo {
        version: 0,
        user: String::new(),
        token: String::new(),
        note: String::new(),
        enabled: false,
        expiration_time: OffsetDateTime::now_utc(),
        creation_time: OffsetDateTime::now_utc(),
        last_edit_time: OffsetDateTime::now_utc(),
        last_usage_time: OffsetDateTime::now_utc(),
    })
}
