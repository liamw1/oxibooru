use crate::schema::{pool, pool_category, pool_name, pool_post};
use crate::search::{Error, Order, ParsedSort, QueryArgs, SearchCriteria};
use crate::{apply_having_clause, apply_str_filter, apply_time_filter, finalize};
use diesel::dsl::*;
use diesel::pg::Pg;
use diesel::prelude::*;
use strum::EnumString;

pub type BoxedQuery<'a> = IntoBoxed<'a, Select<pool::table, pool::id>, Pg>;

#[derive(Clone, Copy, EnumString)]
#[strum(serialize_all = "kebab-case")]
pub enum Token {
    #[strum(serialize = "creation-date", serialize = "creation-time")]
    CreationTime,
    #[strum(
        serialize = "edit-date",
        serialize = "edit-time",
        serialize = "last-edit-date",
        serialize = "last-edit-time"
    )]
    LastEditTime,

    // Requires join
    Name,
    Category,
    PostCount,
}

pub fn parse_search_criteria(search_criteria: &str) -> Result<SearchCriteria<Token>, Error> {
    SearchCriteria::new(search_criteria, Token::Name)
        .map_err(Box::from)
        .map_err(Error::from)
}

pub fn build_query<'a>(search_criteria: &'a SearchCriteria<Token>) -> Result<BoxedQuery<'a>, Error> {
    let base_query = pool::table.select(pool::id).into_boxed();
    search_criteria
        .filters
        .iter()
        .try_fold(base_query, |query, filter| match filter.kind {
            Token::CreationTime => apply_time_filter!(query, pool::creation_time, filter),
            Token::LastEditTime => apply_time_filter!(query, pool::last_edit_time, filter),
            Token::Name => {
                let names = pool_name::table.select(pool_name::pool_id).into_boxed();
                let subquery = apply_str_filter!(names, pool_name::name, filter);
                Ok(query.filter(pool::id.eq_any(subquery)))
            }
            Token::Category => {
                let category_names = pool::table
                    .select(pool::id)
                    .inner_join(pool_category::table)
                    .into_boxed();
                let subquery = apply_str_filter!(category_names, pool_category::name, filter);
                Ok(query.filter(pool::id.eq_any(subquery)))
            }
            Token::PostCount => {
                let pool_posts = pool::table
                    .select(pool::id)
                    .left_join(pool_post::table)
                    .group_by(pool::id)
                    .into_boxed();
                apply_having_clause!(pool_posts, count(pool_post::post_id), filter)
                    .map(|subquery| query.filter(pool::id.eq_any(subquery)))
            }
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
    let sort = search_criteria.sorts.last().map(|&sort| sort).unwrap_or(ParsedSort {
        kind: Token::CreationTime,
        order: Order::default(),
    });

    match sort.kind {
        Token::CreationTime => finalize!(query, pool::creation_time, sort, extra_args).load(conn),
        Token::LastEditTime => finalize!(query, pool::last_edit_time, sort, extra_args).load(conn),
        Token::Name => name_sorted(conn, query, sort, extra_args),
        Token::Category => category_sorted(conn, query, sort, extra_args),
        Token::PostCount => post_count_sorted(conn, query, sort, extra_args),
    }
}

fn name_sorted(
    conn: &mut PgConnection,
    query: BoxedQuery,
    sort: ParsedSort<Token>,
    extra_args: Option<QueryArgs>,
) -> QueryResult<Vec<i32>> {
    let filtered_pools: Vec<i32> = query.load(conn)?;
    let final_query = pool::table
        .select(pool::id)
        .group_by(pool::id)
        .left_join(pool_name::table)
        .filter(pool::id.eq_any(&filtered_pools))
        .filter(pool_name::order.eq(0)) // Only sort on first name
        .into_boxed();
    finalize!(final_query, min(pool_name::name), sort, extra_args).load(conn)
}

fn category_sorted(
    conn: &mut PgConnection,
    query: BoxedQuery,
    sort: ParsedSort<Token>,
    extra_args: Option<QueryArgs>,
) -> QueryResult<Vec<i32>> {
    let filtered_pools: Vec<i32> = query.load(conn)?;
    let final_query = pool::table
        .select(pool::id)
        .group_by(pool::id)
        .left_join(pool_category::table)
        .filter(pool::id.eq_any(&filtered_pools))
        .into_boxed();
    finalize!(final_query, min(pool_category::name), sort, extra_args).load(conn)
}

fn post_count_sorted(
    conn: &mut PgConnection,
    query: BoxedQuery,
    sort: ParsedSort<Token>,
    extra_args: Option<QueryArgs>,
) -> QueryResult<Vec<i32>> {
    let filtered_pools: Vec<i32> = query.load(conn)?;
    let final_query = pool::table
        .select(pool::id)
        .group_by(pool::id)
        .left_join(pool_post::table)
        .filter(pool::id.eq_any(&filtered_pools))
        .into_boxed();
    finalize!(final_query, count(pool_post::post_id), sort, extra_args).load(conn)
}
