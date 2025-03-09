use crate::model::enums::UserRank;
use crate::schema::{user, user_token};
use crate::time::DateTime;
use crate::{auth, db};
use base64::DecodeError;
use base64::prelude::*;
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
    #[error("Token has expired")]
    InvalidToken,
    #[error("Authentication credentials are malformed")]
    MalformedCredentials,
    MalformedToken(#[from] uuid::Error),
    #[error("Invalid username and password combination")]
    UsernamePasswordMismatch,
    Utf8Conversion(#[from] Utf8Error),
}

#[derive(Clone, Copy)]
pub struct Client {
    pub id: Option<i64>,
    pub rank: UserRank,
}

impl Client {
    pub fn new(id: Option<i64>, rank: UserRank) -> Self {
        Self { id, rank }
    }
}

/// Authentication can either be done by token-based authentication (reccommended)
/// or by sending password as plaintext.
pub fn authenticate_user(auth: String) -> Result<Client, AuthenticationError> {
    let (auth_type, credentials) = auth.split_once(' ').ok_or(AuthenticationError::MalformedCredentials)?;
    match auth_type {
        "Basic" => basic_access_authentication(credentials),
        "Token" => token_authentication(credentials),
        _ => Err(AuthenticationError::InvalidAuthType),
    }
}

#[cfg(test)]
pub fn credentials_for(username: &str, password: &str) -> String {
    let credentials = format!("{username}:{password}");
    BASE64_STANDARD.encode(credentials)
}

/// `credentials` are sent base64 encoded, so this function decodes them to utf-8.
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

/// Checks that the given `credentials` are of the form "username:password"
/// and that the username/password combination is valid.
fn basic_access_authentication(credentials: &str) -> Result<Client, AuthenticationError> {
    let (username, password) = decode_credentials(credentials)?;
    let mut conn = db::get_connection()?;

    // For security reasons, don't give any indication to the user if it was the password
    // or the username that was incorrect.
    let (user_id, rank, password_hash): (i64, UserRank, String) = user::table
        .select((user::id, user::rank, user::password_hash))
        .filter(user::name.eq(username))
        .first(&mut conn)
        .optional()?
        .ok_or(AuthenticationError::UsernamePasswordMismatch)?;
    auth::password::is_valid_password(&password_hash, &password)
        .then_some(Client::new(Some(user_id), rank))
        .ok_or(AuthenticationError::UsernamePasswordMismatch)
}

/// Checks that the given `credentials` are of the form "username:token"
/// and that the username/token combination is valid and non-expired.
fn token_authentication(credentials: &str) -> Result<Client, AuthenticationError> {
    let (username, unparsed_token) = decode_credentials(credentials)?;
    let token = Uuid::parse_str(&unparsed_token)?;

    let mut conn = db::get_connection()?;

    let (user_id, rank, enabled, expiration_time): (i64, UserRank, bool, Option<DateTime>) = user_token::table
        .inner_join(user::table)
        .select((user::id, user::rank, user_token::enabled, user_token::expiration_time))
        .filter(user::name.eq(username))
        .filter(user_token::id.eq(token))
        .first(&mut conn)?;

    let expired = expiration_time.as_ref().is_some_and(|&time| time < DateTime::now());
    let is_valid_token = enabled && !expired;
    is_valid_token
        .then_some(Client::new(Some(user_id), rank))
        .ok_or(AuthenticationError::InvalidToken)
}
