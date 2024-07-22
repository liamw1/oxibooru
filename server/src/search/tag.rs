use crate::schema::{post_tag, tag, tag_category, tag_implication, tag_name, tag_suggestion};
use crate::search::{Error, Order, ParsedSort, UnparsedFilter};
use crate::{apply_having_clause, apply_sort, apply_str_filter, apply_time_filter};
use diesel::define_sql_function;
use diesel::dsl::*;
use diesel::pg::Pg;
use diesel::prelude::*;
use std::str::FromStr;
use strum::EnumString;

pub type BoxedQuery<'a> = IntoBoxed<'a, Select<tag::table, tag::id>, Pg>;

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
            .try_fold(tag::table.select(tag::id).into_boxed(), |query, filter| match filter.kind {
                Token::CreationTime => apply_time_filter!(query, tag::creation_time, filter),
                Token::LastEditTime => apply_time_filter!(query, tag::last_edit_time, filter),
                Token::Name => {
                    let names = tag_name::table.select(tag_name::tag_id).into_boxed();
                    let subquery = apply_str_filter!(names, tag_name::name, filter);
                    Ok(query.filter(tag::id.eq_any(subquery)))
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
                    apply_having_clause!(post_tags, post_tag::post_id, filter)
                        .map(|subquery| query.filter(tag::id.eq_any(subquery)))
                }
                Token::SuggestionCount => {
                    let tag_suggestions = tag::table
                        .select(tag::id)
                        .left_join(tag_suggestion::table)
                        .group_by(tag::id)
                        .into_boxed();
                    apply_having_clause!(tag_suggestions, tag_suggestion::child_id, filter)
                        .map(|subquery| query.filter(tag::id.eq_any(subquery)))
                }
                Token::ImplicationCount => {
                    let tag_implications = tag::table
                        .select(tag::id)
                        .left_join(tag_implication::table)
                        .group_by(tag::id)
                        .into_boxed();
                    apply_having_clause!(tag_implications, tag_implication::child_id, filter)
                        .map(|subquery| query.filter(tag::id.eq_any(subquery)))
                }
            })?;

    // If random sort specified, no other sorts matter
    if random_sort {
        define_sql_function!(fn random() -> Integer);
        return Ok(query.order(random()));
    }
    // Add default sort if none specified
    if sorts.is_empty() {
        sorts.push(ParsedSort {
            kind: Token::CreationTime,
            order: Order::default(),
        })
    }

    Ok(sorts.into_iter().fold(query, |query, sort| match sort.kind {
        Token::CreationTime => apply_sort!(query, tag::creation_time, sort),
        Token::LastEditTime => apply_sort!(query, tag::last_edit_time, sort),
        Token::Name => unimplemented!(),
        Token::Category => unimplemented!(),
        Token::UsageCount => unimplemented!(),
        Token::SuggestionCount => unimplemented!(),
        Token::ImplicationCount => unimplemented!(),
    }))
}

#[derive(Clone, Copy, EnumString)]
#[strum(serialize_all = "kebab-case")]
enum Token {
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
    #[strum(serialize = "name")]
    Name,
    Category,
    #[strum(serialize = "usages", serialize = "post-count", serialize = "usage-count")]
    UsageCount,
    SuggestionCount,
    ImplicationCount,
}
