use crate::model::tag::TagName;
use crate::schema::{tag, tag_category, tag_implication, tag_name, tag_statistics, tag_suggestion};
use crate::search::{Error, Order, ParsedSort, SearchCriteria};
use crate::{apply_filter, apply_sort, apply_str_filter, apply_subquery_filter, apply_time_filter};
use diesel::define_sql_function;
use diesel::dsl::{InnerJoin, IntoBoxed, Select};
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
            Token::UsageCount => apply_filter!(query, tag_statistics::usage_count, filter, i64),
            Token::ImplicationCount => apply_filter!(query, tag_statistics::implication_count, filter, i64),
            Token::SuggestionCount => apply_filter!(query, tag_statistics::suggestion_count, filter, i64),
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
    unsorted_query: BoxedQuery,
    search_criteria: &SearchCriteria<Token>,
) -> QueryResult<Vec<i64>> {
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

    let unsorted_query = unsorted_query.inner_join(tag_name::table).filter(TagName::primary());
    let query = sorts.iter().fold(unsorted_query, |query, sort| match sort.kind {
        Token::CreationTime => apply_sort!(query, tag::creation_time, sort),
        Token::LastEditTime => apply_sort!(query, tag::last_edit_time, sort),
        Token::Name => apply_sort!(query, tag_name::name, sort),
        Token::Category => apply_sort!(query, tag_category::name, sort),
        Token::UsageCount => apply_sort!(query, tag_statistics::usage_count, sort),
        Token::ImplicationCount | Token::HasImplication => apply_sort!(query, tag_statistics::implication_count, sort),
        Token::SuggestionCount | Token::HasSuggestion => apply_sort!(query, tag_statistics::suggestion_count, sort),
    });
    match search_criteria.extra_args {
        Some(args) => query.offset(args.offset).limit(args.limit),
        None => query,
    }
    .load(conn)
}
