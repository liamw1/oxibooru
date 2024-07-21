mod macros;
mod parse;
pub mod post;
pub mod user;

use std::ops::{Not, Range};

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub enum TimeParsingError {
    #[error("Dates need at least one parameter")]
    TooFewArgs,
    #[error("Dates can have at most one parameter")]
    TooManyArgs,
    NotAnInteger(#[from] std::num::ParseIntError),
    OutOfRange(#[from] time::error::ComponentRange),
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub enum Error {
    ParseFailed(#[from] Box<dyn std::error::Error>),
    InvalidTime(#[from] TimeParsingError),
    #[error("This operation requires you to be logged in")]
    NotLoggedIn,
}

#[derive(Clone, Copy)]
enum Order {
    Asc,
    Desc,
}

impl Default for Order {
    fn default() -> Self {
        Self::Desc
    }
}

impl Not for Order {
    type Output = Self;
    fn not(self) -> Self::Output {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Criteria<V> {
    Values(Vec<V>),
    GreaterEq(V),
    LessEq(V),
    Range(Range<V>),
}

struct UnparsedFilter<'a, T> {
    kind: T,
    criteria: &'a str,
    negated: bool,
}

struct ParsedSort<T> {
    kind: T,
    order: Order,
}
