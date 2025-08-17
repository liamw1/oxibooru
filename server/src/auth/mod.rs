use crate::model::enums::{ResourceType, UserRank};
use thiserror::Error;

pub mod header;
pub mod password;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum AuthenticationError {
    #[error("Token has expired")]
    ExpiredToken,
    FailedConnection(#[from] diesel::r2d2::PoolError),
    FailedQuery(#[from] diesel::result::Error),
    #[error("Invalid authentication type")]
    InvalidAuthType,
    InvalidEncoding(#[from] base64::DecodeError),
    #[error("Authentication credentials are malformed")]
    MalformedCredentials,
    MalformedToken(#[from] uuid::Error),
    #[error("{0} not found")]
    NotFound(ResourceType),
    PasswordHashing(#[from] argon2::password_hash::Error),
    #[error("Invalid username and password combination")]
    UsernamePasswordMismatch,
    Utf8Conversion(#[from] std::str::Utf8Error),
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
