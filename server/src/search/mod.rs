pub mod comment;
mod macros;
mod parse;
pub mod pool;
pub mod post;
pub mod tag;
pub mod user;

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
    #[error("This operation requires you to be logged in")]
    NotLoggedIn,
}

pub struct SearchCriteria<'a, T> {
    filters: Vec<UnparsedFilter<'a, T>>,
    sorts: Vec<ParsedSort<T>>,
    specials: Vec<&'a str>, // Leaving this as unparsed since it makes things more difficult otherwise
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
        let mut specials: Vec<&'a str> = Vec::new();
        let mut random_sort = false;

        for mut term in search_criteria.split_whitespace() {
            let negated = term.chars().nth(0) == Some('-');
            if negated {
                term = term.strip_prefix('-').unwrap();
            }

            match term.split_once(':') {
                Some(("special", value)) => specials.push(value),
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
            specials,
            random_sort,
            extra_args: None,
        })
    }

    pub fn add_offset_and_limit(&mut self, offset: i64, limit: i64) {
        self.extra_args = Some(QueryArgs { offset, limit });
    }

    fn parse_special_tokens<S>(&self) -> Result<Vec<S>, <S as FromStr>::Err>
    where
        S: FromStr,
        <S as FromStr>::Err: std::error::Error,
    {
        self.specials.iter().map(|&special| special).map(S::from_str).collect()
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

struct UnparsedFilter<'a, T> {
    kind: T,
    criteria: &'a str,
    negated: bool,
}

#[derive(Clone, Copy)]
struct ParsedSort<T> {
    kind: T,
    order: Order,
}
