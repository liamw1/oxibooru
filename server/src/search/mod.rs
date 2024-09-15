pub mod comment;
mod macros;
mod parse;
pub mod pool;
pub mod post;
pub mod tag;
pub mod user;

use std::borrow::Cow;
use std::ops::{Not, Range};
use std::str::FromStr;

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
    #[error("Invalid sort token")]
    InvalidSort,
    #[error("This operation requires you to be logged in")]
    NotLoggedIn,
}

pub struct SearchCriteria<'a, T> {
    filters: Vec<UnparsedFilter<'a, T>>,
    sorts: Vec<ParsedSort<T>>,
    random_sort: bool,
    extra_args: Option<QueryArgs>,
}

impl<'a, T> SearchCriteria<'a, T>
where
    T: Copy,
    T: FromStr,
    <T as FromStr>::Err: std::error::Error,
{
    pub fn new(search_criteria: &'a str, anonymous_token: T) -> Result<Self, <T as FromStr>::Err> {
        let mut filters: Vec<UnparsedFilter<T>> = Vec::new();
        let mut sorts: Vec<ParsedSort<T>> = Vec::new();
        let mut random_sort = false;

        for mut term in search_criteria.split_whitespace() {
            let negated = term.chars().nth(0) == Some('-');
            if negated {
                term = term.strip_prefix('-').unwrap();
            }

            match parse::split_once_escaped(term, ":") {
                Some(("sort", "random")) => random_sort = true,
                Some(("sort", value)) => {
                    let kind = T::from_str(value)?;
                    let order = if negated { !Order::default() } else { Order::default() };
                    sorts.push(ParsedSort { kind, order });
                }
                Some((key, criteria)) => {
                    filters.push(UnparsedFilter {
                        kind: T::from_str(key)?,
                        criteria,
                        negated,
                    });
                }
                None => filters.push(UnparsedFilter {
                    kind: anonymous_token,
                    criteria: term,
                    negated,
                }),
            }
        }

        Ok(Self {
            filters,
            sorts,
            random_sort,
            extra_args: None,
        })
    }

    pub fn add_offset_and_limit(&mut self, offset: i64, limit: i64) {
        self.extra_args = Some(QueryArgs { offset, limit });
    }

    pub fn has_no_sort(&self) -> bool {
        self.sorts.is_empty()
    }
}

#[derive(Clone, Copy)]
struct QueryArgs {
    offset: i64,
    limit: i64,
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

/*
    String filters can also have wildcards.
    Only one allowed pattern per filter for now.
*/
#[derive(Debug, PartialEq, Eq)]
enum StrCritera<'a> {
    Regular(Criteria<Cow<'a, str>>),
    WildCard(String),
}

#[derive(Clone, Copy)]
struct UnparsedFilter<'a, T> {
    kind: T,
    criteria: &'a str,
    negated: bool,
}

impl<'a, T> UnparsedFilter<'a, T> {
    fn unnegated(self) -> Self {
        Self {
            kind: self.kind,
            criteria: self.criteria,
            negated: false,
        }
    }
}

#[derive(Clone, Copy)]
struct ParsedSort<T> {
    kind: T,
    order: Order,
}
