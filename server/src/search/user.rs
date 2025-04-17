use crate::api::ApiResult;
use crate::schema::user;
use crate::search::{Order, ParsedSort, SearchCriteria};
use crate::{api, apply_random_sort, apply_sort, apply_str_filter, apply_time_filter};
use diesel::dsl::{IntoBoxed, Select};
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

pub fn parse_search_criteria(search_criteria: &str) -> ApiResult<SearchCriteria<Token>> {
    SearchCriteria::new(search_criteria, Token::Name)
        .map_err(Box::from)
        .map_err(api::Error::from)
}

pub fn build_query<'a>(search: &'a SearchCriteria<Token>) -> ApiResult<BoxedQuery<'a>> {
    let base_query = user::table.select(user::id).into_boxed();
    search
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
    unsorted_query: BoxedQuery,
    search: &SearchCriteria<Token>,
) -> QueryResult<Vec<i64>> {
    // If random sort specified, no other sorts matter
    if search.random_sort {
        return apply_random_sort!(unsorted_query, search).load(conn);
    }

    let default_sort = std::iter::once(ParsedSort {
        kind: Token::Name,
        order: Order::default(),
    });
    let sorts = search.sorts.iter().copied().chain(default_sort);
    let query = sorts.fold(unsorted_query, |query, sort| match sort.kind {
        Token::Name => apply_sort!(query, user::name, sort),
        Token::CreationTime => apply_sort!(query, user::creation_time, sort),
        Token::LastLoginTime => apply_sort!(query, user::last_login_time, sort),
    });
    match search.extra_args {
        Some(args) => query.offset(args.offset).limit(args.limit),
        None => query,
    }
    .load(conn)
}
