use crate::schema::{tag, tag_category, tag_implication, tag_name, tag_statistics, tag_suggestion};
use crate::search::{Error, Order, ParsedSort, SearchCriteria};
use crate::{apply_filter, apply_str_filter, apply_subquery_filter, apply_time_filter, finalize};
use diesel::define_sql_function;
use diesel::dsl::*;
use diesel::pg::Pg;
use diesel::prelude::*;
use strum::EnumString;

pub type BoxedQuery<'a> =
    IntoBoxed<'a, InnerJoin<InnerJoin<Select<tag::table, tag::id>, tag_statistics::table>, tag_category::table>, Pg>;

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
    let base_query = tag::table
        .select(tag::id)
        .inner_join(tag_statistics::table)
        .inner_join(tag_category::table)
        .into_boxed();
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
            Token::Category => Ok(apply_str_filter!(query, tag_category::name, filter)),
            Token::UsageCount => apply_filter!(query, tag_statistics::usage_count, filter, i32),
            Token::ImplicationCount => apply_filter!(query, tag_statistics::implication_count, filter, i32),
            Token::SuggestionCount => apply_filter!(query, tag_statistics::suggestion_count, filter, i32),
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
    let query = query.inner_join(tag_name::table).filter(tag_name::order.eq(0));

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
        Token::Name => finalize!(query, tag_name::name, sort, extra_args).load(conn),
        Token::Category => finalize!(query, tag_category::name, sort, extra_args).load(conn),
        Token::UsageCount => finalize!(query, tag_statistics::usage_count, sort, extra_args).load(conn),
        Token::ImplicationCount | Token::HasImplication => {
            finalize!(query, tag_statistics::implication_count, sort, extra_args).load(conn)
        }
        Token::SuggestionCount | Token::HasSuggestion => {
            finalize!(query, tag_statistics::suggestion_count, sort, extra_args).load(conn)
        }
    }
}
