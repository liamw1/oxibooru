use crate::auth;
use crate::model::user::{User, UserToken};
use crate::schema::user_token;
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
    FailedConnection(#[from] diesel::ConnectionError),
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
    #[error("Authentication credentials contain non-ASCII characters")]
    NonAsciiCredentials,
    Utf8Conversion(#[from] Utf8Error),
}

pub fn authenticate_user(auth: String) -> Result<User, AuthenticationError> {
    let (auth_type, credentials) = auth.split_once(' ').ok_or(AuthenticationError::MalformedCredentials)?;
    match auth_type {
        "Basic" => basic_access_authentication(credentials),
        "Token" => token_authentication(credentials),
        _ => Err(AuthenticationError::InvalidAuthType),
    }
}

fn decode_credentials(credentials: &str) -> Result<(String, String), AuthenticationError> {
    let ascii_encoded_b64 = credentials
        .chars()
        .map(|c| c.is_ascii().then_some(c as u8))
        .collect::<Option<Vec<_>>>()
        .ok_or(AuthenticationError::NonAsciiCredentials)?;
    let decoded_credentials = BASE64_STANDARD.decode(ascii_encoded_b64)?;
    let utf8_encoded_credentials = decoded_credentials
        .split(|&c| c == b':')
        .map(std::str::from_utf8)
        .collect::<Result<Vec<_>, _>>()?;
    utf8_encoded_credentials
        .into_iter()
        .map(|s| s.to_owned())
        .collect_tuple()
        .ok_or(AuthenticationError::MalformedCredentials)
}

fn basic_access_authentication(credentials: &str) -> Result<User, AuthenticationError> {
    let (username, password) = decode_credentials(credentials)?;

    let mut conn = crate::establish_connection()?;
    let user = User::from_name(&mut conn, &username)?;
    auth::password::is_valid_password(&user, &password)
        .then_some(user)
        .ok_or(AuthenticationError::InvalidPassword)
}

fn token_authentication(credentials: &str) -> Result<User, AuthenticationError> {
    let (username, unparsed_token) = decode_credentials(credentials)?;
    let token = Uuid::parse_str(&unparsed_token)?;

    let mut conn = crate::establish_connection()?;
    let user = User::from_name(&mut conn, &username)?;
    let user_token = UserToken::belonging_to(&user)
        .select(UserToken::as_select())
        .filter(user_token::token.eq(token))
        .first(&mut conn)?;
    auth::token::is_valid_token(&user_token)
        .then_some(user)
        .ok_or(AuthenticationError::InvalidToken)
}
