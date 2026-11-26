use crate::model::enums::UserRank;
use crate::string::SmallString;

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

#[derive(Clone, Default)]
pub struct ClientName(Option<SmallString>);

impl ClientName {
    pub fn anon() -> Self {
        Self(None)
    }

    pub fn as_deref(&self) -> Option<&str> {
        self.0.as_deref()
    }
}
