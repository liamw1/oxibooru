use crate::api::ApiResult;
use crate::auth::Client;
use argon2::password_hash::rand_core::{OsRng, RngCore};
use diesel::sql_types::{Float, SingleValue};
use diesel::{ExpressionMethods, PgConnection, QueryDsl, QueryResult, RunQueryDsl, declare_sql_function};
use std::borrow::Cow;
use std::ops::{Not, Range};
use std::str::FromStr;

pub mod comment;
mod macros;
mod parse;
pub mod pool;
pub mod post;
pub mod snapshot;
pub mod tag;
mod temp;
pub mod user;

/// An interface for a search query builder.
pub trait Builder<'a> {
    type Token;
    type BoxedQuery;

    /// Returns a stored [`SearchCriteria`] as a mutable reference.
    fn criteria(&mut self) -> &mut SearchCriteria<'a, Self::Token>;

    /// Executes a query that returns primary keys of all rows matching search criteria.
    fn load(&mut self, conn: &mut PgConnection) -> ApiResult<Vec<i64>>;

    /// Executes a count query for the number of rows matching search criteria.
    fn count(&mut self, conn: &mut PgConnection) -> ApiResult<i64>;

    /// Sets OFFSET and LIMIT for search query.
    fn set_offset_and_limit(&mut self, offset: i64, limit: i64) {
        self.criteria().set_offset_and_limit(offset, limit);
    }

    /// Executes both load and count queries for all rows matching search criteria.
    /// If the search has a random sort, then this will also mutate the user's search seed.
    fn list(&mut self, conn: &mut PgConnection) -> ApiResult<(i64, Vec<i64>)> {
        if self.criteria().random_sort {
            change_user_seed(conn, self.criteria().client)?;
        }

        let total = self.count(conn)?;
        let results = self.load(conn)?;
        Ok((total, results))
    }
}

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
    client: Client,
    filters: Vec<UnparsedFilter<'a, T>>,
    sorts: Vec<ParsedSort<T>>,
    random_sort: bool,
    extra_args: Option<QueryArgs>,
}

impl<T> SearchCriteria<'_, T> {
    pub fn has_sort(&self) -> bool {
        !self.sorts.is_empty() || self.random_sort
    }

    pub fn has_filter(&self) -> bool {
        !self.filters.is_empty()
    }

    fn set_offset_and_limit(&mut self, offset: i64, limit: i64) {
        self.extra_args = Some(QueryArgs { offset, limit });
    }
}

impl<'a, T> SearchCriteria<'a, T>
where
    T: Copy + FromStr,
    <T as FromStr>::Err: std::error::Error,
{
    /// Constructs a new [`SearchCriteria`] by parsing `search_criteria` [`str`] into a set terms
    /// (filters or sorts) separated by unescaped whitespace. If a term does not contain an unescaped `:`,
    /// then it will be interpreted as an `anonymous_token`.
    fn new(client: Client, search_criteria: &'a str, anonymous_token: T) -> Result<Self, <T as FromStr>::Err> {
        let mut filters: Vec<UnparsedFilter<T>> = Vec::new();
        let mut sorts: Vec<ParsedSort<T>> = Vec::new();
        let mut random_sort = false;

        // Terms are separated by whitespace
        for term in parse::split_unescaped_whitespace(search_criteria) {
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
            client,
            filters,
            sorts,
            random_sort,
            extra_args: None,
        })
    }
}

#[derive(Clone, Copy)]
struct QueryArgs {
    offset: i64,
    limit: i64,
}

#[derive(Default, Clone, Copy)]
enum Order {
    Asc,
    #[default]
    Desc,
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
    /// Returns a version of the filter where `negated` is `false`.
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

/// Stores the current state of the filter cache TEMP tables.
/// `has_matching` indicates if the [`temp::matching`] table has values.
/// `has_nonmatching` indicates if the [`temp::nonmatching`] table has values.
/// `completed` indicates if the cache for this query has been fully built.
struct CacheState {
    has_matching: bool,
    has_nonmatching: bool,
    completed: bool,
}

impl CacheState {
    fn new() -> Self {
        Self {
            has_matching: false,
            has_nonmatching: false,
            completed: false,
        }
    }
}

#[declare_sql_function]
extern "SQL" {
    fn random() -> BigInt;
}

#[declare_sql_function]
extern "SQL" {
    fn lower<T: SingleValue>(text: T) -> Text;
}

/// Sets the global postgresql random seed to the search seed of the `client`.
fn set_seed(conn: &mut PgConnection, client: Client) -> QueryResult<()> {
    use crate::schema::user;

    let seed = match client.id {
        Some(user_id) => user::table.find(user_id).select(user::search_seed).first(conn)?,
        None => 0.0,
    };
    diesel::sql_query("SELECT setseed($1);")
        .bind::<Float, _>(seed)
        .execute(conn)
        .map(|_| ())
}

/// Cycles search seed for `client` to a new random seed.
fn change_user_seed(conn: &mut PgConnection, client: Client) -> QueryResult<()> {
    use crate::schema::user;

    if let Some(user_id) = client.id {
        let rng = &mut OsRng;
        let random_i32 = i32::from_le_bytes(rng.next_u32().to_le_bytes());
        let new_seed = f64::from(random_i32) / f64::from(i32::MAX);
        diesel::update(user::table.find(user_id))
            .set(user::search_seed.eq(new_seed as f32))
            .execute(conn)?;
    }
    Ok(())
}
