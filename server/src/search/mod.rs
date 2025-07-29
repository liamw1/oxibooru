use diesel::define_sql_function;
use diesel::sql_types::SingleValue;
use std::borrow::Cow;
use std::collections::HashSet;
use std::ops::{Not, Range};
use std::str::FromStr;

pub mod comment;
mod macros;
mod parse;
pub mod pool;
pub mod post;
pub mod snapshot;
pub mod tag;
pub mod user;

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

/// Stores filters, sorts, offset, and limit of a search query.
/// Filters will be parsed later.
pub struct SearchCriteria<'a, T> {
    filters: Vec<UnparsedFilter<'a, T>>,
    sorts: Vec<ParsedSort<T>>,
    random_sort: bool,
    extra_args: Option<QueryArgs>,
}

impl<'a, T> SearchCriteria<'a, T>
where
    T: Copy + FromStr,
    <T as FromStr>::Err: std::error::Error,
{
    pub fn has_sort(&self) -> bool {
        !self.sorts.is_empty() || self.random_sort
    }

    pub fn has_filter(&self) -> bool {
        !self.filters.is_empty()
    }

    pub fn has_random_sort(&self) -> bool {
        self.random_sort
    }

    fn new(search_criteria: &'a str, anonymous_token: T) -> Result<Self, <T as FromStr>::Err> {
        let mut filters: Vec<UnparsedFilter<T>> = Vec::new();
        let mut sorts: Vec<ParsedSort<T>> = Vec::new();
        let mut random_sort = false;

        // Filters are separated by whitespace
        for term in search_criteria.split_whitespace() {
            let (term, negated) = match term.strip_prefix('-') {
                Some(unnegated_term) => (unnegated_term, true),
                None => (term, false),
            };

            match parse::split_once(term, ':') {
                Some(("sort", "random")) => random_sort = true,
                Some(("sort", value)) => {
                    let (token, direction) = match value.split_once(',') {
                        Some((kind, "asc")) => (kind, Order::Asc),
                        Some((kind, "desc")) => (kind, Order::Desc),
                        _ => (value, Order::default()),
                    };

                    let kind = T::from_str(token)?;
                    let order = if negated { !direction } else { direction };
                    sorts.push(ParsedSort { kind, order });
                }
                Some((key, condition)) => {
                    filters.push(UnparsedFilter {
                        kind: T::from_str(key)?,
                        condition,
                        negated,
                    });
                }
                None => filters.push(UnparsedFilter {
                    kind: anonymous_token,
                    condition: term,
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

    fn set_offset_and_limit(&mut self, offset: i64, limit: i64) {
        self.extra_args = Some(QueryArgs { offset, limit });
    }
}

define_sql_function!(fn random() -> BigInt);
define_sql_function!(fn lower<T: SingleValue>(text: T) -> Text);

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

/// Represents a parsed filter on a column or expression.
#[derive(Debug, PartialEq, Eq)]
enum Condition<V> {
    Values(Vec<V>),
    GreaterEq(V),
    LessEq(V),
    Range(Range<V>),
}

/// Represents a parsed filter on a string-based column or expression.
/// Can either be the usual filter or a wildcard filter.
/// Only one allowed wildcard pattern per filter for now.
#[derive(Debug, PartialEq, Eq)]
enum StrCondition<'a> {
    Regular(Condition<Cow<'a, str>>),
    WildCard(String),
}

/// Represents an unparsed filter on a column or expression.
#[derive(Clone, Copy)]
struct UnparsedFilter<'a, T> {
    kind: T,
    condition: &'a str,
    negated: bool,
}

impl<T> UnparsedFilter<'_, T> {
    fn unnegated(self) -> Self {
        Self {
            kind: self.kind,
            condition: self.condition,
            negated: false,
        }
    }

    // Checks if condition represents multiple values (i.e. a range or wildcard pattern).
    fn is_multivalued(&self) -> bool {
        self.condition.contains('*') || self.condition.contains("..")
    }
}

/// Represents a parsed ordering on a column or expression.
#[derive(Clone, Copy)]
struct ParsedSort<T> {
    kind: T,
    order: Order,
}

/// A cache that stores results from what would otherwise be subqueries in preparation for a
/// search query. This is used in place of subqueries because `PostgreSQL` does a poor job of
/// optimizing queries that contain multiple subquery filters.
struct QueryCache {
    matches: Option<HashSet<i64>>,
    nonmatches: Option<HashSet<i64>>,
}

impl QueryCache {
    /// Creates an empty cache
    fn new() -> Self {
        Self {
            matches: None,
            nonmatches: None,
        }
    }

    /// Returns a new [`QueryCache`] if `self` is empty and [None] otherwise.
    /// This function exists because of aliasing issues in mutable `QueryBuilder` functions.
    fn clone_if_empty(&self) -> Option<Self> {
        let is_empty = self.matches.is_none() && self.nonmatches.is_none();
        is_empty.then(Self::new)
    }

    /// If `value` is [Some], replaces content of `self` with a semantically equivalent [`QueryCache`].
    /// Does nothing if `value` is [None].
    fn replace(&mut self, value: Option<Self>) {
        if let Some(cache) = value {
            // If matching and nonmatching sets both exist, we can subtract the nonmatching
            // set from the matching set and discard the nonmatching set. This makes queries
            // that have both positive and negative conditions much faster.
            let (matches, nonmatches) = match (cache.matches, cache.nonmatches) {
                (Some(match_set), Some(nonmatch_set)) => {
                    let set_difference = match_set.difference(&nonmatch_set).copied().collect();
                    (Some(set_difference), None)
                }
                (matches, nonmatches) => (matches, nonmatches),
            };
            *self = Self { matches, nonmatches };
        }
    }

    /// Updates `self` with a new batch of `post_ids`, which may represents
    /// a matching (`negated = false`) or nonmatching (`negated = true`) set of posts.
    fn update(&mut self, post_ids: Vec<i64>, negated: bool) {
        // Nonmatching sets are unioned while matching sets are intersected
        if negated {
            self.nonmatches.get_or_insert_default().extend(post_ids);
        } else {
            self.matches = Some(match self.matches.as_ref() {
                Some(matches) => post_ids.into_iter().filter(|id| matches.contains(id)).collect(),
                None => post_ids.into_iter().collect(),
            });
        }
    }
}
