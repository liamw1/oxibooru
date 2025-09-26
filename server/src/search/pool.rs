use crate::api::ApiResult;
use crate::auth::Client;
use crate::model::pool::PoolName;
use crate::schema::{database_statistics, pool, pool_category, pool_name, pool_statistics};
use crate::search::{Order, ParsedSort, QueryCache, SearchCriteria, UnparsedFilter};
use crate::{
    api, apply_distinct_if_multivalued, apply_filter, apply_random_sort, apply_sort, apply_str_filter,
    apply_time_filter, search,
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
    PostCount,
}

pub struct QueryBuilder<'a> {
    client: Client,
    search: SearchCriteria<'a, Token>,
    cache: QueryCache,
}

impl<'a> QueryBuilder<'a> {
    pub fn new(client: Client, search_criteria: &'a str) -> ApiResult<Self> {
        let search = SearchCriteria::new(search_criteria, Token::Name).map_err(Box::from)?;
        Ok(Self {
            client,
            search,
            cache: QueryCache::new(),
        })
    }

    pub fn set_offset_and_limit(&mut self, offset: i64, limit: i64) {
        self.search.set_offset_and_limit(offset, limit);
    }

    pub fn load(&mut self, conn: &mut PgConnection) -> ApiResult<Vec<i64>> {
        let query = self.build_filtered(conn)?;
        let query = self.apply_cache_filters(query);
        self.get_ordered_ids(conn, query).map_err(api::Error::from)
    }

    pub fn list(&mut self, conn: &mut PgConnection) -> ApiResult<(i64, Vec<i64>)> {
        if self.search.random_sort {
            search::change_seed(conn, self.client)?;
        }

        let total = self.count(conn)?;
        let results = self.load(conn)?;
        Ok((total, results))
    }

    fn build_filtered(&mut self, conn: &mut PgConnection) -> ApiResult<BoxedQuery<'a>> {
        let mut cache = self.cache.clone_if_empty();
        let base_query = pool::table
            .select(pool::id)
            .inner_join(pool_statistics::table)
            .inner_join(pool_category::table)
            .into_boxed();
        let query = self
            .search
            .filters
            .iter()
            .try_fold(base_query, |query, &filter| match filter.kind {
                Token::CreationTime => apply_time_filter!(query, pool::creation_time, filter),
                Token::LastEditTime => apply_time_filter!(query, pool::last_edit_time, filter),
                Token::Name => apply_name_filter(conn, query, filter, cache.as_mut()),
                Token::Category => Ok(apply_str_filter!(query, pool_category::name, filter)),
                Token::PostCount => apply_filter!(query, pool_statistics::post_count, filter, i64),
            })?;
        self.cache.replace(cache);
        Ok(query)
    }

    fn get_ordered_ids(&self, conn: &mut PgConnection, unsorted_query: BoxedQuery<'a>) -> QueryResult<Vec<i64>> {
        // If random sort specified, no other sorts matter
        if self.search.random_sort {
            return apply_random_sort!(conn, self.client, unsorted_query, self.search).load(conn);
        }

        let default_sort = std::iter::once(ParsedSort {
            kind: Token::CreationTime,
            order: Order::default(),
        });
        let sorts = self.search.sorts.iter().copied().chain(default_sort);
        let unsorted_query = unsorted_query.inner_join(pool_name::table).filter(PoolName::primary());
        let query = sorts.fold(unsorted_query, |query, sort| match sort.kind {
            Token::CreationTime => apply_sort!(query, pool::creation_time, sort),
            Token::LastEditTime => apply_sort!(query, pool::last_edit_time, sort),
            Token::Name => apply_sort!(query, pool_name::name, sort),
            Token::Category => apply_sort!(query, pool_category::name, sort),
            Token::PostCount => apply_sort!(query, pool_statistics::post_count, sort),
        });
        match self.search.extra_args {
            Some(args) => query.offset(args.offset).limit(args.limit),
            None => query,
        }
        .load(conn)
    }

    fn apply_cache_filters(&'a self, mut query: BoxedQuery<'a>) -> BoxedQuery<'a> {
        if let Some(matching_ids) = self.cache.matches.as_ref() {
            query = query.filter(pool::id.eq_any(matching_ids));
        }
        if let Some(nonmatching_ids) = self.cache.nonmatches.as_ref() {
            query = query.filter(pool::id.ne_all(nonmatching_ids));
        }
        query
    }

    fn count(&mut self, conn: &mut PgConnection) -> ApiResult<i64> {
        if self.search.has_filter() {
            let unsorted_query = self.build_filtered(conn)?;
            let unsorted_query = self.apply_cache_filters(unsorted_query);
            unsorted_query.count().first(conn)
        } else {
            database_statistics::table
                .select(database_statistics::pool_count)
                .first(conn)
        }
        .map_err(api::Error::from)
    }
}

type BoxedQuery<'a> = IntoBoxed<
    'a,
    InnerJoin<InnerJoin<Select<pool::table, pool::id>, pool_statistics::table>, pool_category::table>,
    Pg,
>;

fn apply_name_filter<'a>(
    conn: &mut PgConnection,
    query: BoxedQuery<'a>,
    filter: UnparsedFilter<Token>,
    cache: Option<&mut QueryCache>,
) -> ApiResult<BoxedQuery<'a>> {
    if let Some(cache) = cache {
        let names = pool_name::table.select(pool_name::pool_id).into_boxed();
        let names = apply_distinct_if_multivalued!(names, filter);
        let filtered_pools = apply_str_filter!(names, pool_name::name, filter.unnegated());
        let pool_ids: Vec<i64> = filtered_pools.load(conn)?;
        cache.update(pool_ids, filter.negated);
    }
    Ok(query)
}
