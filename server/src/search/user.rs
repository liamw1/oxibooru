use crate::schema::user;
use crate::search::{Error, Order, ParsedSort, UnparsedFilter};
use crate::{apply_sort, apply_str_filter, apply_time_filter};
use diesel::define_sql_function;
use diesel::dsl::{IntoBoxed, Select};
use diesel::pg::Pg;
use diesel::prelude::*;
use std::str::FromStr;
use strum::EnumString;

pub type BoxedQuery<'a> = IntoBoxed<'a, Select<user::table, user::id>, Pg>;

pub fn build_query(search_criteria: &str) -> Result<BoxedQuery, Error> {
    let mut filters: Vec<UnparsedFilter<Token>> = Vec::new();
    let mut sorts: Vec<ParsedSort<Token>> = Vec::new();
    let mut random_sort = false;

    for mut term in search_criteria.split_whitespace() {
        let negated = term.chars().nth(0) == Some('-');
        if negated {
            term = term.strip_prefix('-').unwrap();
        }

        match term.split_once(':') {
            Some(("sort", "random")) => random_sort = true,
            Some(("sort", value)) => {
                let kind = Token::from_str(value).map_err(Box::from)?;
                let order = if negated { !Order::default() } else { Order::default() };
                sorts.push(ParsedSort { kind, order });
            }
            Some((key, criteria)) => {
                filters.push(UnparsedFilter {
                    kind: Token::from_str(key).map_err(Box::from)?,
                    criteria,
                    negated,
                });
            }
            None => filters.push(UnparsedFilter {
                kind: Token::Name,
                criteria: term,
                negated,
            }),
        }
    }

    let query =
        filters
            .into_iter()
            .try_fold(user::table.select(user::id).into_boxed(), |query, filter| match filter.kind {
                Token::Name => Ok(apply_str_filter!(query, user::name, filter)),
                Token::CreationTime => apply_time_filter!(query, user::creation_time, filter),
                Token::LastLoginTime => apply_time_filter!(query, user::last_login_time, filter),
            })?;

    // If random sort specified, no other sorts matter
    if random_sort {
        define_sql_function!(fn random() -> Integer);
        return Ok(query.order_by(random()));
    }
    // Add default sort if none specified
    if sorts.is_empty() {
        sorts.push(ParsedSort {
            kind: Token::Name,
            order: Order::default(),
        })
    }

    Ok(sorts.into_iter().fold(query, |query, sort| match sort.kind {
        Token::Name => apply_sort!(query, user::name, sort),
        Token::CreationTime => apply_sort!(query, user::creation_time, sort),
        Token::LastLoginTime => apply_sort!(query, user::last_login_time, sort),
    }))
}

#[derive(Clone, Copy, EnumString)]
enum Token {
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
