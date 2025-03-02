use crate::schema::{comment, comment_statistics, user};
use crate::search::{Error, Order, ParsedSort, SearchCriteria};
use crate::{apply_filter, apply_sort, apply_str_filter, apply_time_filter};
use diesel::dsl::{InnerJoin, IntoBoxed, LeftJoin, Select};
use diesel::pg::Pg;
use diesel::prelude::*;
use strum::EnumString;

pub type BoxedQuery<'a> =
    IntoBoxed<'a, LeftJoin<InnerJoin<Select<comment::table, comment::id>, comment_statistics::table>, user::table>, Pg>;

#[derive(Clone, Copy, EnumString)]
#[strum(serialize_all = "kebab-case")]
pub enum Token {
    Id,
    Post,
    Text,
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
    #[strum(serialize = "user", serialize = "author")]
    User,
    Score,
}

pub fn parse_search_criteria(search_criteria: &str) -> Result<SearchCriteria<Token>, Error> {
    SearchCriteria::new(search_criteria, Token::Text)
        .map_err(Box::from)
        .map_err(Error::from)
}

pub fn build_query<'a>(search_criteria: &'a SearchCriteria<Token>) -> Result<BoxedQuery<'a>, Error> {
    let base_query = comment::table
        .select(comment::id)
        .inner_join(comment_statistics::table)
        .left_join(user::table)
        .into_boxed();
    search_criteria
        .filters
        .iter()
        .try_fold(base_query, |query, filter| match filter.kind {
            Token::Id => apply_filter!(query, comment::id, filter, i64),
            Token::Post => apply_filter!(query, comment::post_id, filter, i64),
            Token::Text => Ok(apply_str_filter!(query, comment::text, filter)),
            Token::CreationTime => apply_time_filter!(query, comment::creation_time, filter),
            Token::LastEditTime => apply_time_filter!(query, comment::last_edit_time, filter),
            Token::User => Ok(apply_str_filter!(query, user::name, filter)),
            Token::Score => apply_filter!(query, comment_statistics::score, filter, i64),
        })
}

pub fn get_ordered_ids(
    conn: &mut PgConnection,
    unsorted_query: BoxedQuery,
    search_criteria: &SearchCriteria<Token>,
) -> QueryResult<Vec<i64>> {
    // If random sort specified, no other sorts matter
    if search_criteria.random_sort {
        define_sql_function!(fn random() -> BigInt);
        return match search_criteria.extra_args {
            Some(args) => unsorted_query.order(random()).offset(args.offset).limit(args.limit),
            None => unsorted_query.order(random()),
        }
        .load(conn);
    }

    let default_sort = std::iter::once(ParsedSort {
        kind: Token::CreationTime,
        order: Order::default(),
    });
    let sorts = search_criteria.sorts.iter().copied().chain(default_sort);
    let query = sorts.fold(unsorted_query, |query, sort| match sort.kind {
        Token::Id => apply_sort!(query, comment::id, sort),
        Token::Post => apply_sort!(query, comment::post_id, sort),
        Token::Text => apply_sort!(query, comment::text, sort),
        Token::CreationTime => apply_sort!(query, comment::creation_time, sort),
        Token::LastEditTime => apply_sort!(query, comment::last_edit_time, sort),
        Token::User => apply_sort!(query, user::name, sort),
        Token::Score => apply_sort!(query, comment_statistics::score, sort),
    });
    match search_criteria.extra_args {
        Some(args) => query.offset(args.offset).limit(args.limit),
        None => query,
    }
    .load(conn)
}
