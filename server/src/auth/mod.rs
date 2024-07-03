pub mod content;
pub mod header;
pub mod password;
pub mod token;

use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum HashError {
    EnvVar(#[from] std::env::VarError),
    Hash(#[from] argon2::password_hash::Error),
}
