use crate::model::enums::UserRank;

pub mod header;
pub mod password;

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
