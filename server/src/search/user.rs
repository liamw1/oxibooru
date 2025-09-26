use crate::api::ApiResult;
use crate::auth::Client;
use crate::schema::{database_statistics, user};
use crate::search::{Order, ParsedSort, SearchCriteria};
use crate::{api, apply_random_sort, apply_sort, apply_str_filter, apply_time_filter, search};
use diesel::dsl::{IntoBoxed, Select};
use diesel::pg::Pg;
use diesel::prelude::*;
use strum::EnumString;

#[derive(Clone, Copy, EnumString)]
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
    client: Client,
    search: SearchCriteria<'a, Token>,
}

impl<'a> QueryBuilder<'a> {
    pub fn new(client: Client, search_criteria: &'a str) -> ApiResult<Self> {
        let search = SearchCriteria::new(search_criteria, Token::Name).map_err(Box::from)?;
        Ok(Self { client, search })
    }

    pub fn set_offset_and_limit(&mut self, offset: i64, limit: i64) {
        self.search.set_offset_and_limit(offset, limit);
    }

    pub fn load(&mut self, conn: &mut PgConnection) -> ApiResult<Vec<i64>> {
        let query = self.build_filtered()?;
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

    fn build_filtered(&mut self) -> ApiResult<BoxedQuery<'a>> {
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

    fn get_ordered_ids(&self, conn: &mut PgConnection, unsorted_query: BoxedQuery<'a>) -> QueryResult<Vec<i64>> {
        // If random sort specified, no other sorts matter
        if self.search.random_sort {
            return apply_random_sort!(conn, self.client, unsorted_query, self.search).load(conn);
        }

        let default_sort = std::iter::once(ParsedSort {
            kind: Token::Name,
            order: Order::default(),
        });
        let sorts = self.search.sorts.iter().copied().chain(default_sort);
        let query = sorts.fold(unsorted_query, |query, sort| match sort.kind {
            Token::Name => apply_sort!(query, user::name, sort),
            Token::CreationTime => apply_sort!(query, user::creation_time, sort),
            Token::LastLoginTime => apply_sort!(query, user::last_login_time, sort),
        });
        match self.search.extra_args {
            Some(args) => query.offset(args.offset).limit(args.limit),
            None => query,
        }
        .load(conn)
    }

    fn count(&mut self, conn: &mut PgConnection) -> ApiResult<i64> {
        if self.search.has_filter() {
            let unsorted_query = self.build_filtered()?;
            unsorted_query.count().first(conn)
        } else {
            database_statistics::table
                .select(database_statistics::user_count)
                .first(conn)
        }
        .map_err(api::Error::from)
    }
}

type BoxedQuery<'a> = IntoBoxed<'a, Select<user::table, user::id>, Pg>;
