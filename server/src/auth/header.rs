use crate::model::user::{User, UserToken};
use crate::schema::user_token;
use crate::{auth, db};
use base64::prelude::*;
use base64::DecodeError;
use diesel::prelude::*;
use itertools::Itertools;
use std::str::Utf8Error;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum AuthenticationError {
    FailedConnection(#[from] diesel::r2d2::PoolError),
    FailedQuery(#[from] diesel::result::Error),
    #[error("Invalid authentication type")]
    InvalidAuthType,
    InvalidEncoding(#[from] DecodeError),
    #[error("Password is incorrect")]
    InvalidPassword,
    #[error("Token has expired")]
    InvalidToken,
    #[error("Authentication credentials are malformed")]
    MalformedCredentials,
    MalformedToken(#[from] uuid::Error),
    Utf8Conversion(#[from] Utf8Error),
}

/*
    Authentication can either be done by token-based authentication (reccommended)
    or by sending password as plaintext.
*/
pub fn authenticate_user(auth: String) -> Result<User, AuthenticationError> {
    let (auth_type, credentials) = auth.split_once(' ').ok_or(AuthenticationError::MalformedCredentials)?;
    match auth_type {
        "Basic" => basic_access_authentication(credentials),
        "Token" => token_authentication(credentials),
        _ => Err(AuthenticationError::InvalidAuthType),
    }
}

/*
    Credentials are sent base64 encoded, so this function decodes them to utf-8.
*/
fn decode_credentials(credentials: &str) -> Result<(String, String), AuthenticationError> {
    let decoded_credentials = BASE64_STANDARD.decode(credentials)?;
    let utf8_encoded_credentials = decoded_credentials
        .split(|&c| c == b':')
        .map(std::str::from_utf8)
        .collect::<Result<Vec<_>, _>>()?;
    utf8_encoded_credentials
        .into_iter()
        .map(str::to_owned)
        .collect_tuple()
        .ok_or(AuthenticationError::MalformedCredentials)
}

/*
    Checks that the given credentials are of the form "username:password"
    and that the username/password combination is valid.
*/
fn basic_access_authentication(credentials: &str) -> Result<User, AuthenticationError> {
    let (username, password) = decode_credentials(credentials)?;

    let mut conn = db::get_connection()?;
    let user = User::from_name(&mut conn, &username)?;
    auth::password::is_valid_password(&user, &password)
        .then_some(user)
        .ok_or(AuthenticationError::InvalidPassword)
}

/*
    Checks that the given credentials are of the form "username:token"
    and that the username/token combination is valid and non-expired.
*/
fn token_authentication(credentials: &str) -> Result<User, AuthenticationError> {
    let (username, unparsed_token) = decode_credentials(credentials)?;
    let token = Uuid::parse_str(&unparsed_token)?;

    let mut conn = db::get_connection()?;
    let user = User::from_name(&mut conn, &username)?;
    let user_token = UserToken::belonging_to(&user)
        .filter(user_token::token.eq(token))
        .first(&mut conn)?;
    auth::token::is_valid_token(&user_token)
        .then_some(user)
        .ok_or(AuthenticationError::InvalidToken)
}
