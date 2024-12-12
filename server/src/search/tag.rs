use crate::schema::{post_tag, tag, tag_category, tag_implication, tag_name, tag_suggestion};
use crate::search::{Error, Order, ParsedSort, QueryArgs, SearchCriteria};
use crate::{apply_having_clause, apply_str_filter, apply_subquery_filter, apply_time_filter, finalize};
use diesel::define_sql_function;
use diesel::dsl::*;
use diesel::pg::Pg;
use diesel::prelude::*;
use strum::EnumString;

pub type BoxedQuery<'a> = IntoBoxed<'a, Select<tag::table, tag::id>, Pg>;

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
    #[strum(serialize = "usages", serialize = "post-count", serialize = "usage-count")]
    UsageCount,
    ImplicationCount,
    SuggestionCount,
    HasImplication,
    HasSuggestion,
}

pub fn parse_search_criteria(search_criteria: &str) -> Result<SearchCriteria<Token>, Error> {
    SearchCriteria::new(search_criteria, Token::Name)
        .map_err(Box::from)
        .map_err(Error::from)
}

pub fn build_query<'a>(search_criteria: &'a SearchCriteria<Token>) -> Result<BoxedQuery<'a>, Error> {
    let base_query = tag::table.select(tag::id).into_boxed();
    search_criteria
        .filters
        .iter()
        .try_fold(base_query, |query, filter| match filter.kind {
            Token::CreationTime => apply_time_filter!(query, tag::creation_time, filter),
            Token::LastEditTime => apply_time_filter!(query, tag::last_edit_time, filter),
            Token::Name => {
                let names = tag_name::table.select(tag_name::tag_id).into_boxed();
                let subquery = apply_str_filter!(names, tag_name::name, filter.unnegated());
                Ok(apply_subquery_filter!(query, tag::id, filter, subquery))
            }
            Token::Category => {
                let category_names = tag::table.select(tag::id).inner_join(tag_category::table).into_boxed();
                let subquery = apply_str_filter!(category_names, tag_category::name, filter);
                Ok(query.filter(tag::id.eq_any(subquery)))
            }
            Token::UsageCount => {
                let post_tags = tag::table
                    .select(tag::id)
                    .left_join(post_tag::table)
                    .group_by(tag::id)
                    .into_boxed();
                apply_having_clause!(post_tags, count(post_tag::post_id), filter)
                    .map(|subquery| query.filter(tag::id.eq_any(subquery)))
            }
            Token::ImplicationCount => {
                let tag_implications = tag::table
                    .select(tag::id)
                    .left_join(tag_implication::table)
                    .group_by(tag::id)
                    .into_boxed();
                apply_having_clause!(tag_implications, count(tag_implication::child_id), filter)
                    .map(|subquery| query.filter(tag::id.eq_any(subquery)))
            }
            Token::SuggestionCount => {
                let tag_suggestions = tag::table
                    .select(tag::id)
                    .left_join(tag_suggestion::table)
                    .group_by(tag::id)
                    .into_boxed();
                apply_having_clause!(tag_suggestions, count(tag_suggestion::child_id), filter)
                    .map(|subquery| query.filter(tag::id.eq_any(subquery)))
            }
            Token::HasImplication => {
                let implications = tag_implication::table
                    .select(tag_implication::parent_id)
                    .inner_join(tag_name::table.on(tag_implication::child_id.eq(tag_name::tag_id)))
                    .into_boxed();
                let subquery = apply_str_filter!(implications, tag_name::name, filter.unnegated());
                Ok(apply_subquery_filter!(query, tag::id, filter, subquery))
            }
            Token::HasSuggestion => {
                let suggestions = tag_suggestion::table
                    .select(tag_suggestion::parent_id)
                    .inner_join(tag_name::table.on(tag_suggestion::child_id.eq(tag_name::tag_id)))
                    .into_boxed();
                let subquery = apply_str_filter!(suggestions, tag_name::name, filter.unnegated());
                Ok(apply_subquery_filter!(query, tag::id, filter, subquery))
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
        Token::CreationTime => finalize!(query, tag::creation_time, sort, extra_args).load(conn),
        Token::LastEditTime => finalize!(query, tag::last_edit_time, sort, extra_args).load(conn),
        Token::Name => name_sorted(conn, query, sort, extra_args),
        Token::Category => category_sorted(conn, query, sort, extra_args),
        Token::UsageCount => usage_count_sorted(conn, query, sort, extra_args),
        Token::ImplicationCount | Token::HasImplication => implication_count_sorted(conn, query, sort, extra_args),
        Token::SuggestionCount | Token::HasSuggestion => suggestion_count_sorted(conn, query, sort, extra_args),
    }
}

fn name_sorted(
    conn: &mut PgConnection,
    query: BoxedQuery,
    sort: ParsedSort<Token>,
    extra_args: Option<QueryArgs>,
) -> QueryResult<Vec<i32>> {
    let filtered_tags: Vec<i32> = query.load(conn)?;
    let final_query = tag::table
        .select(tag::id)
        .group_by(tag::id)
        .left_join(tag_name::table)
        .filter(tag::id.eq_any(&filtered_tags))
        .filter(tag_name::order.eq(0)) // Only sort on first name
        .into_boxed();
    finalize!(final_query, min(tag_name::name), sort, extra_args).load(conn)
}

fn category_sorted(
    conn: &mut PgConnection,
    query: BoxedQuery,
    sort: ParsedSort<Token>,
    extra_args: Option<QueryArgs>,
) -> QueryResult<Vec<i32>> {
    let filtered_tags: Vec<i32> = query.load(conn)?;
    let final_query = tag::table
        .select(tag::id)
        .group_by(tag::id)
        .left_join(tag_category::table)
        .filter(tag::id.eq_any(&filtered_tags))
        .into_boxed();
    finalize!(final_query, min(tag_category::name), sort, extra_args).load(conn)
}

fn usage_count_sorted(
    conn: &mut PgConnection,
    query: BoxedQuery,
    sort: ParsedSort<Token>,
    extra_args: Option<QueryArgs>,
) -> QueryResult<Vec<i32>> {
    let filtered_tags: Vec<i32> = query.load(conn)?;
    let final_query = tag::table
        .select(tag::id)
        .group_by(tag::id)
        .left_join(post_tag::table)
        .filter(tag::id.eq_any(&filtered_tags))
        .into_boxed();
    finalize!(final_query, count(post_tag::post_id), sort, extra_args).load(conn)
}

fn implication_count_sorted(
    conn: &mut PgConnection,
    query: BoxedQuery,
    sort: ParsedSort<Token>,
    extra_args: Option<QueryArgs>,
) -> QueryResult<Vec<i32>> {
    let filtered_tags: Vec<i32> = query.load(conn)?;
    let final_query = tag::table
        .select(tag::id)
        .group_by(tag::id)
        .left_join(tag_implication::table)
        .filter(tag::id.eq_any(&filtered_tags))
        .into_boxed();
    finalize!(final_query, count(tag_implication::child_id), sort, extra_args).load(conn)
}

fn suggestion_count_sorted(
    conn: &mut PgConnection,
    query: BoxedQuery,
    sort: ParsedSort<Token>,
    extra_args: Option<QueryArgs>,
) -> QueryResult<Vec<i32>> {
    let filtered_tags: Vec<i32> = query.load(conn)?;
    let final_query = tag::table
        .select(tag::id)
        .group_by(tag::id)
        .left_join(tag_suggestion::table)
        .filter(tag::id.eq_any(&filtered_tags))
        .into_boxed();
    finalize!(final_query, count(tag_suggestion::child_id), sort, extra_args).load(conn)
}
