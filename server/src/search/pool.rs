use crate::api::error::{ApiError, ApiResult};
use crate::auth::Client;
use crate::model::pool::PoolName;
use crate::schema::{database_statistics, pool, pool_category, pool_name, pool_statistics};
use crate::search::{Builder, CacheState, Order, ParsedSort, SearchCriteria, UnparsedFilter};
use crate::{
    apply_cache_filters, apply_distinct_if_multivalued, apply_filter, apply_random_sort, apply_sort, apply_str_filter,
    apply_time_filter, update_filter_cache,
};
use diesel::dsl::{InnerJoin, IntoBoxed, Select};
use diesel::pg::Pg;
use diesel::{ExpressionMethods, PgConnection, QueryDsl, QueryResult, RunQueryDsl};
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
    search: SearchCriteria<'a, Token>,
    cache_state: CacheState,
}

impl<'a> Builder<'a> for QueryBuilder<'a> {
    type Token = Token;
    type BoxedQuery = BoxedQuery<'a>;

    fn criteria(&mut self) -> &mut SearchCriteria<'a, Self::Token> {
        &mut self.search
    }

    fn load(&mut self, conn: &mut PgConnection) -> ApiResult<Vec<i64>> {
        let query = self.build_filtered(conn)?;
        self.get_ordered_ids(conn, query).map_err(ApiError::from)
    }

    fn count(&mut self, conn: &mut PgConnection) -> ApiResult<i64> {
        if self.search.has_filter() {
            let unsorted_query = self.build_filtered(conn)?;
            unsorted_query.count().first(conn)
        } else {
            database_statistics::table
                .select(database_statistics::pool_count)
                .first(conn)
        }
        .map_err(ApiError::from)
    }
}

impl<'a> QueryBuilder<'a> {
    pub fn new(client: Client, search_criteria: &'a str) -> ApiResult<Self> {
        let search = SearchCriteria::new(client, search_criteria, Token::Name).map_err(Box::from)?;
        Ok(Self {
            search,
            cache_state: CacheState::new(),
        })
    }

    fn build_filtered(&mut self, conn: &mut PgConnection) -> ApiResult<BoxedQuery<'a>> {
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
                Token::Name => apply_name_filter(conn, query, filter, &mut self.cache_state),
                Token::Category => Ok(apply_str_filter!(query, pool_category::name, filter)),
                Token::PostCount => apply_filter!(query, pool_statistics::post_count, filter, i64),
            })?;
        Ok(apply_cache_filters!(query, pool::id, self.cache_state))
    }

    fn get_ordered_ids(&self, conn: &mut PgConnection, unsorted_query: BoxedQuery<'a>) -> QueryResult<Vec<i64>> {
        // If random sort specified, no other sorts matter
        if self.search.random_sort {
            return apply_random_sort!(conn, self.search.client, unsorted_query, self.search).load(conn);
        }

        let default_sort = std::iter::once(ParsedSort {
            kind: Token::CreationTime,
            order: Order::default(),
        });
        let sorts = self.search.sorts.iter().copied().chain(default_sort);
        let unsorted_query = unsorted_query.inner_join(pool_name::table).filter(PoolName::primary());
        let query = sorts.fold(unsorted_query, |query, sort| match sort.kind {
            Token::CreationTime => apply_sort!(query, pool::id, sort),
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
    state: &mut CacheState,
) -> ApiResult<BoxedQuery<'a>> {
    let names = pool_name::table.select(pool_name::pool_id).into_boxed();
    let names = apply_distinct_if_multivalued!(names, filter);
    let filtered_pools = apply_str_filter!(names, pool_name::name, filter.unnegated());
    update_filter_cache!(conn, filtered_pools, pool_name::pool_id, filter, state)?;
    Ok(query)
}
