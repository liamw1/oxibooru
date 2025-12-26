use crate::api::error::{ApiError, ApiResult};
use crate::auth::Client;
use crate::config::Config;
use crate::model::enums::UserRank;
use crate::model::tag::TagName;
use crate::schema::{
    database_statistics, tag, tag_category, tag_implication, tag_name, tag_statistics, tag_suggestion,
};
use crate::search::{Builder, CacheState, Order, ParsedSort, SearchCriteria, UnparsedFilter};
use crate::{
    apply_cache_filters, apply_distinct_if_multivalued, apply_filter, apply_random_sort, apply_sort, apply_str_filter,
    apply_time_filter, update_filter_cache,
};
use diesel::dsl::{InnerJoin, IntoBoxed, Select};
use diesel::pg::Pg;
use diesel::{ExpressionMethods, JoinOnDsl, PgConnection, QueryDsl, QueryResult, RunQueryDsl};
use strum::{Display, EnumIter, EnumString, EnumTable};

#[derive(Display, Clone, Copy, EnumTable, EnumIter, EnumString)]
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
    Name,
    Category,
    Description,
    #[strum(serialize = "usages", serialize = "post-count", serialize = "usage-count")]
    UsageCount,
    ImplicationCount,
    SuggestionCount,
    Implies,
    Suggests,
}

pub struct QueryBuilder<'a> {
    search: SearchCriteria<'a, Token>,
    cache_state: CacheState,
}

impl<'a> Builder<'a> for QueryBuilder<'a> {
    type Token = Token;
    type BoxedQuery = BoxedQuery;

    fn criteria(&mut self) -> &mut SearchCriteria<'a, Self::Token> {
        &mut self.search
    }

    fn count(&mut self, conn: &mut PgConnection) -> ApiResult<i64> {
        if self.search.has_filter() {
            let unsorted_query = self.build_filtered(conn)?;
            unsorted_query.count().first(conn)
        } else {
            database_statistics::table
                .select(database_statistics::tag_count)
                .first(conn)
        }
        .map_err(ApiError::from)
    }

    fn build_filtered(&mut self, conn: &mut PgConnection) -> ApiResult<BoxedQuery> {
        let base_query = tag::table
            .select(tag::id)
            .inner_join(tag_statistics::table)
            .inner_join(tag_category::table)
            .into_boxed();
        let query = self
            .search
            .filters
            .iter()
            .try_fold(base_query, |query, &filter| match filter.kind {
                Token::CreationTime => apply_time_filter!(query, tag::creation_time, filter),
                Token::LastEditTime => apply_time_filter!(query, tag::last_edit_time, filter),
                Token::Name => apply_name_filter(conn, query, filter, &mut self.cache_state),
                Token::Category => Ok(apply_str_filter!(query, tag_category::name, filter)),
                Token::Description => Ok(apply_str_filter!(query, tag::description, filter)),
                Token::UsageCount => apply_filter!(query, tag_statistics::usage_count, filter, i64),
                Token::ImplicationCount => apply_filter!(query, tag_statistics::implication_count, filter, i64),
                Token::SuggestionCount => apply_filter!(query, tag_statistics::suggestion_count, filter, i64),
                Token::Implies => apply_implies_filter(conn, query, filter, &mut self.cache_state),
                Token::Suggests => apply_suggests_filter(conn, query, filter, &mut self.cache_state),
            })?;
        Ok(apply_cache_filters!(query, tag::id, self.cache_state))
    }

    fn get_ordered_ids(&self, conn: &mut PgConnection, unsorted_query: BoxedQuery) -> QueryResult<Vec<i64>> {
        // If random sort specified, no other sorts matter
        if self.search.random_sort {
            return apply_random_sort!(conn, self.search.client, unsorted_query, self.search).load(conn);
        }

        let default_sort = std::iter::once(ParsedSort {
            kind: Token::CreationTime,
            order: Order::default(),
        });
        let sorts = self.search.sorts.iter().copied().chain(default_sort);
        let unsorted_query = unsorted_query.inner_join(tag_name::table).filter(TagName::primary());
        let query = sorts.fold(unsorted_query, |query, sort| match sort.kind {
            Token::CreationTime => apply_sort!(query, tag::id, sort),
            Token::LastEditTime => apply_sort!(query, tag::last_edit_time, sort),
            Token::Name => apply_sort!(query, tag_name::name, sort),
            Token::Category => apply_sort!(query, tag_category::name, sort),
            Token::Description => apply_sort!(query, tag::description, sort),
            Token::UsageCount => apply_sort!(query, tag_statistics::usage_count, sort),
            Token::ImplicationCount | Token::Implies => {
                apply_sort!(query, tag_statistics::implication_count, sort)
            }
            Token::SuggestionCount | Token::Suggests => apply_sort!(query, tag_statistics::suggestion_count, sort),
        });
        match self.search.extra_args {
            Some(args) => query.offset(args.offset).limit(args.limit),
            None => query,
        }
        .load(conn)
    }
}

impl<'a> QueryBuilder<'a> {
    pub fn new(config: &'a Config, client: Client, search_criteria: &'a str) -> ApiResult<Self> {
        let mut search = SearchCriteria::new(client, search_criteria, Token::Name).map_err(Box::from)?;
        if client.rank == UserRank::Anonymous {
            let preferences = &config.anonymous_preferences;

            let tag_blacklist_filters = preferences.tag_blacklist.iter().map(|condition| UnparsedFilter {
                kind: Token::Name,
                condition,
                negated: true,
            });
            search.filters.extend(tag_blacklist_filters);

            let category_blacklist_filters =
                preferences
                    .tag_category_blacklist
                    .iter()
                    .map(|condition| UnparsedFilter {
                        kind: Token::Category,
                        condition,
                        negated: true,
                    });
            search.filters.extend(category_blacklist_filters);
        }

        Ok(Self {
            search,
            cache_state: CacheState::new(),
        })
    }
}

type BoxedQuery = IntoBoxed<
    'static,
    InnerJoin<InnerJoin<Select<tag::table, tag::id>, tag_statistics::table>, tag_category::table>,
    Pg,
>;

fn apply_name_filter(
    conn: &mut PgConnection,
    query: BoxedQuery,
    filter: UnparsedFilter<Token>,
    state: &mut CacheState,
) -> ApiResult<BoxedQuery> {
    let names = tag_name::table.select(tag_name::tag_id).into_boxed();
    let names = apply_distinct_if_multivalued!(names, filter);
    let filtered_tags = apply_str_filter!(names, tag_name::name, filter.unnegated());
    update_filter_cache!(conn, filtered_tags, tag_name::tag_id, filter, state)?;
    Ok(query)
}

fn apply_implies_filter(
    conn: &mut PgConnection,
    query: BoxedQuery,
    filter: UnparsedFilter<Token>,
    state: &mut CacheState,
) -> ApiResult<BoxedQuery> {
    let implications = tag_implication::table
        .select(tag_implication::parent_id)
        .inner_join(tag_name::table.on(tag_implication::child_id.eq(tag_name::tag_id)))
        .into_boxed();
    let implications = apply_distinct_if_multivalued!(implications, filter);
    let filtered_tags = apply_str_filter!(implications, tag_name::name, filter.unnegated());
    update_filter_cache!(conn, filtered_tags, tag_name::tag_id, filter, state)?;
    Ok(query)
}

fn apply_suggests_filter(
    conn: &mut PgConnection,
    query: BoxedQuery,
    filter: UnparsedFilter<Token>,
    state: &mut CacheState,
) -> ApiResult<BoxedQuery> {
    let suggestions = tag_suggestion::table
        .select(tag_suggestion::parent_id)
        .inner_join(tag_name::table.on(tag_suggestion::child_id.eq(tag_name::tag_id)))
        .into_boxed();
    let suggestions = apply_distinct_if_multivalued!(suggestions, filter);
    let filtered_tags = apply_str_filter!(suggestions, tag_name::name, filter.unnegated());
    update_filter_cache!(conn, filtered_tags, tag_name::tag_id, filter, state)?;
    Ok(query)
}

#[cfg(test)]
pub fn filter_table() -> TokenTable<&'static str> {
    TokenTable {
        _creation_time: "-1984",
        _last_edit_time: "1984",
        _name: "*sky*",
        _category: "character",
        _description: "*\\ *",
        _usage_count: "0,2,5",
        _implication_count: "-0",
        _suggestion_count: "-2..",
        _implies: "-sky",
        _suggests: "-*k*",
    }
}
