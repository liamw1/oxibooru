use crate::api::error::{ApiError, ApiResult};
use crate::auth::Client;
use crate::model::enums::{ResourceOperation, ResourceType};
use crate::schema::{snapshot, user};
use crate::search::{Builder, Order, ParsedSort, SearchCriteria};
use crate::{apply_filter, apply_random_sort, apply_sort, apply_str_filter, apply_time_filter};
use diesel::dsl::{IntoBoxed, LeftJoin, Select};
use diesel::pg::Pg;
use diesel::{ExpressionMethods, PgConnection, QueryDsl, QueryResult, RunQueryDsl};
use strum::EnumString;

#[derive(Clone, Copy, EnumString)]
#[strum(serialize_all = "kebab-case")]
pub enum Token {
    User,
    Operation,
    ResourceType,
    #[strum(serialize = "id")]
    ResourceId,
    #[strum(serialize = "date", serialize = "time")]
    Time,
}

pub struct QueryBuilder<'a> {
    search: SearchCriteria<'a, Token>,
}

impl<'a> Builder<'a> for QueryBuilder<'a> {
    type Token = Token;
    type BoxedQuery = BoxedQuery;

    fn criteria(&mut self) -> &mut SearchCriteria<'a, Self::Token> {
        &mut self.search
    }

    fn count(&mut self, conn: &mut PgConnection) -> ApiResult<i64> {
        let unsorted_query = self.build_filtered(conn)?;
        unsorted_query.count().first(conn).map_err(ApiError::from)
    }

    fn build_filtered(&mut self, _conn: &mut PgConnection) -> ApiResult<BoxedQuery> {
        let base_query = snapshot::table.select(snapshot::id).left_join(user::table).into_boxed();
        self.search
            .filters
            .iter()
            .try_fold(base_query, |query, filter| match filter.kind {
                Token::User => Ok(apply_str_filter!(query, user::name, filter)),
                Token::Operation => apply_filter!(query, snapshot::operation, filter, ResourceOperation),
                Token::ResourceType => apply_filter!(query, snapshot::resource_type, filter, ResourceType),
                Token::ResourceId => Ok(apply_str_filter!(query, snapshot::resource_id, filter)),
                Token::Time => apply_time_filter!(query, snapshot::creation_time, filter),
            })
    }

    fn get_ordered_ids(&self, conn: &mut PgConnection, unsorted_query: BoxedQuery) -> QueryResult<Vec<i64>> {
        // If random sort specified, no other sorts matter
        if self.search.random_sort {
            return apply_random_sort!(conn, self.search.client, unsorted_query, self.search).load(conn);
        }

        let default_sort = std::iter::once(ParsedSort {
            kind: Token::Time,
            order: Order::default(),
        });
        let sorts = self.search.sorts.iter().copied().chain(default_sort);
        let query = sorts.fold(unsorted_query, |query, sort| match sort.kind {
            Token::User => apply_sort!(query, user::name, sort),
            Token::Operation => apply_sort!(query, snapshot::operation, sort),
            Token::ResourceType => apply_sort!(query, snapshot::resource_type, sort),
            Token::ResourceId => apply_sort!(query, snapshot::resource_id, sort),
            Token::Time => apply_sort!(query, snapshot::creation_time, sort),
        });
        match self.search.extra_args {
            Some(args) => query.offset(args.offset).limit(args.limit),
            None => query,
        }
        .load(conn)
    }
}

impl<'a> QueryBuilder<'a> {
    pub fn new(client: Client, search_criteria: &'a str) -> ApiResult<Self> {
        let search = SearchCriteria::new(client, search_criteria, Token::ResourceType).map_err(Box::from)?;
        Ok(Self { search })
    }
}

type BoxedQuery = IntoBoxed<'static, LeftJoin<Select<snapshot::table, snapshot::id>, user::table>, Pg>;
