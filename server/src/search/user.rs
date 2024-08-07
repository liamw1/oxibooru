use crate::schema::user;
use crate::search::{Error, Order, ParsedSort, SearchCriteria};
use crate::{apply_str_filter, apply_time_filter, finalize};
use diesel::define_sql_function;
use diesel::dsl::*;
use diesel::pg::Pg;
use diesel::prelude::*;
use strum::EnumString;

pub type BoxedQuery<'a> = IntoBoxed<'a, Select<user::table, user::id>, Pg>;

#[derive(Clone, Copy, EnumString)]
pub enum Token {
    #[strum(serialize = "name")]
    Name,
    #[strum(serialize = "creation-date", serialize = "creation-time")]
    CreationTime,
    #[strum(
        serialize = "login-date",
        serialize = "login-time",
        serialize = "last-login-date",
        serialize = "last-login-time"
    )]
    LastLoginTime,
}

pub fn parse_search_criteria(search_criteria: &str) -> Result<SearchCriteria<Token>, Error> {
    SearchCriteria::new(search_criteria, Token::Name)
        .map_err(Box::from)
        .map_err(Error::from)
}

pub fn build_query<'a>(search_criteria: &'a SearchCriteria<Token>) -> Result<BoxedQuery<'a>, Error> {
    let base_query = user::table.select(user::id).into_boxed();
    search_criteria
        .filters
        .iter()
        .try_fold(base_query, |query, filter| match filter.kind {
            Token::Name => Ok(apply_str_filter!(query, user::name, filter)),
            Token::CreationTime => apply_time_filter!(query, user::creation_time, filter),
            Token::LastLoginTime => apply_time_filter!(query, user::last_login_time, filter),
        })
}

pub fn get_ordered_ids(
    conn: &mut PgConnection,
    query: BoxedQuery,
    search_criteria: &SearchCriteria<Token>,
) -> QueryResult<Vec<i32>> {
    // If random sort specified, no other sorts matter
    let extra_args = search_criteria.extra_args;
    if search_criteria.random_sort {
        define_sql_function!(fn random() -> Integer);
        return match extra_args {
            Some(args) => query.order(random()).offset(args.offset).limit(args.limit),
            None => query.order(random()),
        }
        .load(conn);
    }
    // Add default sort if none specified
    let sort = search_criteria.sorts.last().copied().unwrap_or(ParsedSort {
        kind: Token::Name,
        order: Order::default(),
    });

    match sort.kind {
        Token::Name => finalize!(query, user::name, sort, extra_args).load(conn),
        Token::CreationTime => finalize!(query, user::creation_time, sort, extra_args).load(conn),
        Token::LastLoginTime => finalize!(query, user::last_login_time, sort, extra_args).load(conn),
    }
}
