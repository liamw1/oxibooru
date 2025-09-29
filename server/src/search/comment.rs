use crate::api::ApiResult;
use crate::auth::Client;
use crate::schema::{comment, comment_statistics, database_statistics, user};
use crate::search::{Builder, Order, ParsedSort, SearchCriteria};
use crate::{api, apply_filter, apply_random_sort, apply_sort, apply_str_filter, apply_time_filter};
use diesel::dsl::{InnerJoin, IntoBoxed, LeftJoin, Select};
use diesel::pg::Pg;
use diesel::prelude::*;
use strum::EnumString;

#[derive(Clone, Copy, EnumString)]
#[strum(serialize_all = "kebab-case")]
pub enum Token {
    Id,
    Post,
    Text,
    #[strum(serialize = "creation-date", serialize = "creation-time")]
    CreationTime,
    #[strum(
        serialize = "edit-date",
        serialize = "edit-time",
        serialize = "last-edit-date",
        serialize = "last-edit-time"
    )]
    LastEditTime,
    #[strum(serialize = "user", serialize = "author")]
    User,
    Score,
}

pub struct QueryBuilder<'a> {
    search: SearchCriteria<'a, Token>,
}

impl<'a> Builder<'a> for QueryBuilder<'a> {
    type Token = Token;
    type BoxedQuery = BoxedQuery<'a>;

    fn criteria(&mut self) -> &mut SearchCriteria<'a, Self::Token> {
        &mut self.search
    }

    fn load(&mut self, conn: &mut PgConnection) -> ApiResult<Vec<i64>> {
        let query = self.build_filtered()?;
        self.get_ordered_ids(conn, query).map_err(api::Error::from)
    }

    fn count(&mut self, conn: &mut PgConnection) -> ApiResult<i64> {
        if self.search.has_filter() {
            let unsorted_query = self.build_filtered()?;
            unsorted_query.count().first(conn)
        } else {
            database_statistics::table
                .select(database_statistics::comment_count)
                .first(conn)
        }
        .map_err(api::Error::from)
    }
}

impl<'a> QueryBuilder<'a> {
    pub fn new(client: Client, search_criteria: &'a str) -> ApiResult<Self> {
        let search = SearchCriteria::new(client, search_criteria, Token::Text).map_err(Box::from)?;
        Ok(Self { search })
    }

    fn build_filtered(&mut self) -> ApiResult<BoxedQuery<'a>> {
        let base_query = comment::table
            .select(comment::id)
            .inner_join(comment_statistics::table)
            .left_join(user::table)
            .into_boxed();
        self.search
            .filters
            .iter()
            .try_fold(base_query, |query, filter| match filter.kind {
                Token::Id => apply_filter!(query, comment::id, filter, i64),
                Token::Post => apply_filter!(query, comment::post_id, filter, i64),
                Token::Text => Ok(apply_str_filter!(query, comment::text, filter)),
                Token::CreationTime => apply_time_filter!(query, comment::creation_time, filter),
                Token::LastEditTime => apply_time_filter!(query, comment::last_edit_time, filter),
                Token::User => Ok(apply_str_filter!(query, user::name, filter)),
                Token::Score => apply_filter!(query, comment_statistics::score, filter, i64),
            })
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
        let query = sorts.fold(unsorted_query, |query, sort| match sort.kind {
            Token::Id => apply_sort!(query, comment::id, sort),
            Token::Post => apply_sort!(query, comment::post_id, sort),
            Token::Text => apply_sort!(query, comment::text, sort),
            Token::CreationTime => apply_sort!(query, comment::creation_time, sort),
            Token::LastEditTime => apply_sort!(query, comment::last_edit_time, sort),
            Token::User => apply_sort!(query, user::name, sort),
            Token::Score => apply_sort!(query, comment_statistics::score, sort),
        });
        match self.search.extra_args {
            Some(args) => query.offset(args.offset).limit(args.limit),
            None => query,
        }
        .load(conn)
    }
}

type BoxedQuery<'a> =
    IntoBoxed<'a, LeftJoin<InnerJoin<Select<comment::table, comment::id>, comment_statistics::table>, user::table>, Pg>;
