pub mod hash;

use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum AuthenticationError {
    EnvVar(#[from] std::env::VarError),
    Hash(#[from] argon2::password_hash::Error),
}
