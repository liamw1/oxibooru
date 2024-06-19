pub mod hash;

use crate::config::CONFIG;
use crate::model::rank::UserRank;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum AuthenticationError {
    EnvVar(#[from] std::env::VarError),
    Hash(#[from] argon2::password_hash::Error),
}

pub fn privilege_needed(action_name: &str) -> Option<UserRank> {
    CONFIG
        .get("public_info")
        .and_then(|info| info.get("privileges"))
        .and_then(|table| table.get(action_name))
        .and_then(|parsed| parsed.as_str())
        .and_then(|name| UserRank::from_str(name).ok())
}
