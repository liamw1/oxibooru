pub mod header;
pub mod password;

use crate::model::enums::UserRank;
use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum HashError {
    EnvVar(#[from] std::env::VarError),
    Hash(#[from] argon2::password_hash::Error),
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
