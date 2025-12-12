use crate::api::error::{ApiError, ApiResult};
use crate::auth::Client;
use crate::schema::{database_statistics, user};
use crate::search::{Builder, Order, ParsedSort, SearchCriteria};
use crate::{apply_random_sort, apply_sort, apply_str_filter, apply_time_filter};
use diesel::dsl::{IntoBoxed, Select};
use diesel::pg::Pg;
use diesel::{ExpressionMethods, PgConnection, QueryDsl, QueryResult, RunQueryDsl};
use strum::{Display, EnumIter, EnumString, EnumTable};

#[derive(Display, Clone, Copy, EnumTable, EnumIter, EnumString)]
pub enum Token {
    #[strum(serialize = "name")]
    Name,
    #[strum(serialize = "creation-date", serialize = "creation-time")]
    CreationTime,
    #[strum(
        serialize = "login-date",
        serialize = "login-time",
        serialize = "last-login-date",
        serialize = "last-login-time"
    )]
    LastLoginTime,
}

pub struct QueryBuilder<'a> {
    search: SearchCriteria<'a, Token>,
}

impl<'a> Builder<'a> for QueryBuilder<'a> {
    type Token = Token;
    type BoxedQuery = BoxedQuery;

    fn new(client: Client, search_criteria: &'a str) -> ApiResult<Self> {
        let search = SearchCriteria::new(client, search_criteria, Token::Name).map_err(Box::from)?;
        Ok(Self { search })
    }

    fn criteria(&mut self) -> &mut SearchCriteria<'a, Self::Token> {
        &mut self.search
    }

    fn count(&mut self, conn: &mut PgConnection) -> ApiResult<i64> {
        if self.search.has_filter() {
            let unsorted_query = self.build_filtered(conn)?;
            unsorted_query.count().first(conn)
        } else {
            database_statistics::table
                .select(database_statistics::user_count)
                .first(conn)
        }
        .map_err(ApiError::from)
    }

    fn build_filtered(&mut self, _conn: &mut PgConnection) -> ApiResult<BoxedQuery> {
        let base_query = user::table.select(user::id).into_boxed();
        self.search
            .filters
            .iter()
            .try_fold(base_query, |query, filter| match filter.kind {
                Token::Name => Ok(apply_str_filter!(query, user::name, filter)),
                Token::CreationTime => apply_time_filter!(query, user::creation_time, filter),
                Token::LastLoginTime => apply_time_filter!(query, user::last_login_time, filter),
            })
    }

    fn get_ordered_ids(&self, conn: &mut PgConnection, unsorted_query: BoxedQuery) -> QueryResult<Vec<i64>> {
        // If random sort specified, no other sorts matter
        if self.search.random_sort {
            return apply_random_sort!(conn, self.search.client, unsorted_query, self.search).load(conn);
        }

        let default_sort = std::iter::once(ParsedSort {
            kind: Token::Name,
            order: Order::default(),
        });
        let sorts = self.search.sorts.iter().copied().chain(default_sort);
        let query = sorts.fold(unsorted_query, |query, sort| match sort.kind {
            Token::Name => apply_sort!(query, user::name, sort),
            Token::CreationTime => apply_sort!(query, user::id, sort),
            Token::LastLoginTime => apply_sort!(query, user::last_login_time, sort),
        });
        match self.search.extra_args {
            Some(args) => query.offset(args.offset).limit(args.limit),
            None => query,
        }
        .load(conn)
    }
}

type BoxedQuery = IntoBoxed<'static, Select<user::table, user::id>, Pg>;

#[cfg(test)]
pub fn filter_table() -> TokenTable<&'static str> {
    TokenTable {
        _name: "*user*",
        _creation_time: "-2000",
        _last_login_time: "2000..2001",
    }
}
