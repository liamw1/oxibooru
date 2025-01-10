use crate::schema::{pool, pool_category, pool_name, pool_statistics};
use crate::search::{Error, Order, ParsedSort, SearchCriteria};
use crate::{apply_filter, apply_sort, apply_str_filter, apply_subquery_filter, apply_time_filter};
use diesel::dsl::*;
use diesel::pg::Pg;
use diesel::prelude::*;
use strum::EnumString;

pub type BoxedQuery<'a> = IntoBoxed<
    'a,
    InnerJoin<InnerJoin<Select<pool::table, pool::id>, pool_statistics::table>, pool_category::table>,
    Pg,
>;

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
    let base_query = pool::table
        .select(pool::id)
        .inner_join(pool_statistics::table)
        .inner_join(pool_category::table)
        .into_boxed();
    search_criteria
        .filters
        .iter()
        .try_fold(base_query, |query, filter| match filter.kind {
            Token::CreationTime => apply_time_filter!(query, pool::creation_time, filter),
            Token::LastEditTime => apply_time_filter!(query, pool::last_edit_time, filter),
            Token::Name => {
                let names = pool_name::table.select(pool_name::pool_id).into_boxed();
                let subquery = apply_str_filter!(names, pool_name::name, filter.unnegated());
                Ok(apply_subquery_filter!(query, pool::id, filter, subquery))
            }
            Token::Category => Ok(apply_str_filter!(query, pool_category::name, filter)),
            Token::PostCount => apply_filter!(query, pool_statistics::post_count, filter, i32),
        })
}

pub fn get_ordered_ids(
    conn: &mut PgConnection,
    unsorted_query: BoxedQuery,
    search_criteria: &SearchCriteria<Token>,
) -> QueryResult<Vec<i32>> {
    // If random sort specified, no other sorts matter
    if search_criteria.random_sort {
        define_sql_function!(fn random() -> Integer);
        return match search_criteria.extra_args {
            Some(args) => unsorted_query.order(random()).offset(args.offset).limit(args.limit),
            None => unsorted_query.order(random()),
        }
        .load(conn);
    }

    // Add default sort if none specified
    let sorts = if search_criteria.has_sort() {
        search_criteria.sorts.as_slice()
    } else {
        &[ParsedSort {
            kind: Token::CreationTime,
            order: Order::default(),
        }]
    };

    let unsorted_query = unsorted_query
        .inner_join(pool_name::table)
        .filter(pool_name::order.eq(0));
    let query = sorts.iter().fold(unsorted_query, |query, sort| match sort.kind {
        Token::CreationTime => apply_sort!(query, pool::creation_time, sort),
        Token::LastEditTime => apply_sort!(query, pool::last_edit_time, sort),
        Token::Name => apply_sort!(query, pool_name::name, sort),
        Token::Category => apply_sort!(query, pool_category::name, sort),
        Token::PostCount => apply_sort!(query, pool_statistics::post_count, sort),
    });
    match search_criteria.extra_args {
        Some(args) => query.offset(args.offset).limit(args.limit),
        None => query,
    }
    .load(conn)
}
