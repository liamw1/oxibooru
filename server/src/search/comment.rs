use crate::schema::{comment, user};
use crate::search::{Error, Order, ParsedSort, QueryArgs, SearchCriteria};
use crate::{apply_filter, apply_str_filter, apply_time_filter, finalize};
use diesel::dsl::*;
use diesel::pg::Pg;
use diesel::prelude::*;
use strum::EnumString;

pub type BoxedQuery<'a> = IntoBoxed<'a, Select<comment::table, comment::id>, Pg>;

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
}

pub fn parse_search_criteria(search_criteria: &str) -> Result<SearchCriteria<Token>, Error> {
    SearchCriteria::new(search_criteria, Token::Text)
        .map_err(Box::from)
        .map_err(Error::from)
}

pub fn build_query<'a>(search_criteria: &'a SearchCriteria<Token>) -> Result<BoxedQuery<'a>, Error> {
    let base_query = comment::table.select(comment::id).into_boxed();
    search_criteria
        .filters
        .iter()
        .try_fold(base_query, |query, filter| match filter.kind {
            Token::Id => apply_filter!(query, comment::id, filter, i32),
            Token::Post => apply_filter!(query, comment::post_id, filter, i32),
            Token::Text => Ok(apply_str_filter!(query, comment::text, filter)),
            Token::CreationTime => apply_time_filter!(query, comment::creation_time, filter),
            Token::LastEditTime => apply_time_filter!(query, comment::last_edit_time, filter),
            Token::User => {
                let comment_authors = comment::table.select(comment::id).inner_join(user::table).into_boxed();
                let subquery = apply_str_filter!(comment_authors, user::name, filter);
                Ok(query.filter(comment::id.eq_any(subquery)))
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
    let sort = search_criteria.sorts.last().copied().unwrap_or(ParsedSort {
        kind: Token::CreationTime,
        order: Order::default(),
    });

    match sort.kind {
        Token::Id => finalize!(query, comment::id, sort, extra_args).load(conn),
        Token::Post => finalize!(query, comment::post_id, sort, extra_args).load(conn),
        Token::Text => finalize!(query, comment::text, sort, extra_args).load(conn),
        Token::CreationTime => finalize!(query, comment::creation_time, sort, extra_args).load(conn),
        Token::LastEditTime => finalize!(query, comment::last_edit_time, sort, extra_args).load(conn),
        Token::User => author_sorted(conn, query, sort, extra_args),
    }
}

fn author_sorted(
    conn: &mut PgConnection,
    query: BoxedQuery,
    sort: ParsedSort<Token>,
    extra_args: Option<QueryArgs>,
) -> QueryResult<Vec<i32>> {
    let filtered_comments: Vec<i32> = query.load(conn)?;
    let final_query = comment::table
        .select(comment::id)
        .group_by(comment::id)
        .inner_join(user::table)
        .filter(comment::id.eq_any(&filtered_comments))
        .into_boxed();
    finalize!(final_query, min(user::name), sort, extra_args).load(conn)
}
