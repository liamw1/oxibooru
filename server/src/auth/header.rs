use crate::auth;
use crate::model::user::User;
use base64::prelude::*;
use base64::DecodeError;
use std::str::Utf8Error;
use thiserror::Error;

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
    #[error("Authentication credentials are malformed")]
    MalformedCredentials,
    Utf8Conversion(#[from] Utf8Error),
}

pub fn authenticate_user(auth: String) -> Result<User, AuthenticationError> {
    let (auth_type, credentials) = auth.split_once(' ').ok_or(AuthenticationError::MalformedCredentials)?;
    match auth_type {
        "Basic" => basic_access_authentication(credentials),
        // TODO: Handle "Token"
        _ => Err(AuthenticationError::InvalidAuthType),
    }
}

fn basic_access_authentication(credentials: &str) -> Result<User, AuthenticationError> {
    let ascii_encoded_b64: Vec<u8> = credentials
        .chars()
        .into_iter()
        .filter(|c| c.is_ascii())
        .map(|c| c as u8)
        .collect();
    let decoded_credentials = BASE64_STANDARD.decode(ascii_encoded_b64)?;
    let utf8_encoded_credentials = std::str::from_utf8(&decoded_credentials)?;
    let (username, password) = utf8_encoded_credentials
        .split_once(':')
        .ok_or(AuthenticationError::MalformedCredentials)?;

    let mut conn = crate::establish_connection()?;
    let user = User::from_name(&mut conn, username)?;
    match auth::hash::is_valid_password(&user, password) {
        true => Ok(user),
        false => Err(AuthenticationError::InvalidPassword),
    }
}
