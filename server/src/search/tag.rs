use crate::api::ApiResult;
use crate::model::tag::TagName;
use crate::schema::{
    database_statistics, tag, tag_category, tag_implication, tag_name, tag_statistics, tag_suggestion,
};
use crate::search::{Order, ParsedSort, QueryCache, SearchCriteria, UnparsedFilter};
use crate::{
    api, apply_distinct_if_multivalued, apply_filter, apply_random_sort, apply_sort, apply_str_filter,
    apply_time_filter,
};
use diesel::dsl::{InnerJoin, IntoBoxed, Select};
use diesel::pg::Pg;
use diesel::prelude::*;
use strum::EnumString;

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
    Name,
    Category,
    #[strum(serialize = "usages", serialize = "post-count", serialize = "usage-count")]
    UsageCount,
    ImplicationCount,
    SuggestionCount,
    Implies,
    Suggests,
}

pub struct QueryBuilder<'a> {
    search: SearchCriteria<'a, Token>,
    cache: QueryCache,
}

impl<'a> QueryBuilder<'a> {
    pub fn new(search_criteria: &'a str) -> ApiResult<Self> {
        let search = SearchCriteria::new(search_criteria, Token::Name).map_err(Box::from)?;
        Ok(Self {
            search,
            cache: QueryCache::new(),
        })
    }

    pub fn set_offset_and_limit(&mut self, offset: i64, limit: i64) {
        self.search.set_offset_and_limit(offset, limit);
    }

    pub fn count(&mut self, conn: &mut PgConnection) -> ApiResult<i64> {
        if self.search.has_filter() {
            let unsorted_query = self.build_filtered(conn)?;
            let unsorted_query = self.apply_cache_filters(unsorted_query);
            unsorted_query.count().first(conn)
        } else {
            database_statistics::table
                .select(database_statistics::tag_count)
                .first(conn)
        }
        .map_err(api::Error::from)
    }

    pub fn load(&mut self, conn: &mut PgConnection) -> ApiResult<Vec<i64>> {
        let query = self.build_filtered(conn)?;
        let query = self.apply_cache_filters(query);
        self.get_ordered_ids(conn, query).map_err(api::Error::from)
    }

    fn build_filtered(&mut self, conn: &mut PgConnection) -> ApiResult<BoxedQuery<'a>> {
        let mut cache = self.cache.clone_if_empty();
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
                Token::Name => apply_name_filter(conn, query, filter, cache.as_mut()),
                Token::Category => Ok(apply_str_filter!(query, tag_category::name, filter)),
                Token::UsageCount => apply_filter!(query, tag_statistics::usage_count, filter, i64),
                Token::ImplicationCount => apply_filter!(query, tag_statistics::implication_count, filter, i64),
                Token::SuggestionCount => apply_filter!(query, tag_statistics::suggestion_count, filter, i64),
                Token::Implies => apply_implies_filter(conn, query, filter, cache.as_mut()),
                Token::Suggests => apply_suggests_filter(conn, query, filter, cache.as_mut()),
            })?;
        self.cache.replace(cache);
        Ok(query)
    }

    fn get_ordered_ids(&self, conn: &mut PgConnection, unsorted_query: BoxedQuery<'a>) -> QueryResult<Vec<i64>> {
        // If random sort specified, no other sorts matter
        if self.search.random_sort {
            return apply_random_sort!(unsorted_query, self.search).load(conn);
        }

        let default_sort = std::iter::once(ParsedSort {
            kind: Token::CreationTime,
            order: Order::default(),
        });
        let sorts = self.search.sorts.iter().copied().chain(default_sort);
        let unsorted_query = unsorted_query.inner_join(tag_name::table).filter(TagName::primary());
        let query = sorts.fold(unsorted_query, |query, sort| match sort.kind {
            Token::CreationTime => apply_sort!(query, tag::creation_time, sort),
            Token::LastEditTime => apply_sort!(query, tag::last_edit_time, sort),
            Token::Name => apply_sort!(query, tag_name::name, sort),
            Token::Category => apply_sort!(query, tag_category::name, sort),
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

    fn apply_cache_filters(&'a self, mut query: BoxedQuery<'a>) -> BoxedQuery<'a> {
        if let Some(matching_ids) = self.cache.matches.as_ref() {
            query = query.filter(tag::id.eq_any(matching_ids));
        }
        if let Some(nonmatching_ids) = self.cache.nonmatches.as_ref() {
            query = query.filter(tag::id.ne_all(nonmatching_ids));
        }
        query
    }
}

type BoxedQuery<'a> =
    IntoBoxed<'a, InnerJoin<InnerJoin<Select<tag::table, tag::id>, tag_statistics::table>, tag_category::table>, Pg>;

fn apply_name_filter<'a>(
    conn: &mut PgConnection,
    query: BoxedQuery<'a>,
    filter: UnparsedFilter<Token>,
    cache: Option<&mut QueryCache>,
) -> ApiResult<BoxedQuery<'a>> {
    if let Some(cache) = cache {
        let names = tag_name::table.select(tag_name::tag_id).into_boxed();
        let names = apply_distinct_if_multivalued!(names, filter);
        let filtered_tags = apply_str_filter!(names, tag_name::name, filter.unnegated());
        let tag_ids: Vec<i64> = filtered_tags.load(conn)?;
        cache.update(tag_ids, filter.negated);
    }
    Ok(query)
}

fn apply_implies_filter<'a>(
    conn: &mut PgConnection,
    query: BoxedQuery<'a>,
    filter: UnparsedFilter<Token>,
    cache: Option<&mut QueryCache>,
) -> ApiResult<BoxedQuery<'a>> {
    if let Some(cache) = cache {
        let implications = tag_implication::table
            .select(tag_implication::parent_id)
            .inner_join(tag_name::table.on(tag_implication::child_id.eq(tag_name::tag_id)))
            .into_boxed();
        let implications = apply_distinct_if_multivalued!(implications, filter);
        let filtered_tags = apply_str_filter!(implications, tag_name::name, filter.unnegated());
        let tag_ids: Vec<i64> = filtered_tags.load(conn)?;
        cache.update(tag_ids, filter.negated);
    }
    Ok(query)
}

fn apply_suggests_filter<'a>(
    conn: &mut PgConnection,
    query: BoxedQuery<'a>,
    filter: UnparsedFilter<Token>,
    cache: Option<&mut QueryCache>,
) -> ApiResult<BoxedQuery<'a>> {
    if let Some(cache) = cache {
        let suggestions = tag_suggestion::table
            .select(tag_suggestion::parent_id)
            .inner_join(tag_name::table.on(tag_suggestion::child_id.eq(tag_name::tag_id)))
            .into_boxed();
        let suggestions = apply_distinct_if_multivalued!(suggestions, filter);
        let filtered_tags = apply_str_filter!(suggestions, tag_name::name, filter.unnegated());
        let tag_ids: Vec<i64> = filtered_tags.load(conn)?;
        cache.update(tag_ids, filter.negated);
    }
    Ok(query)
}
