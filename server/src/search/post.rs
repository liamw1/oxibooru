use crate::api::ApiResult;
use crate::auth::Client;
use crate::content::hash::Checksum;
use crate::model::enums::{PostFlag, PostFlags, PostSafety, PostType};
use crate::schema::{
    comment, database_statistics, pool_post, post, post_favorite, post_feature, post_note, post_score, post_statistics,
    post_tag, tag_name, user,
};
use crate::search::{Builder, Order, ParsedSort, QueryCache, SearchCriteria, UnparsedFilter, parse};
use crate::{
    api, apply_distinct_if_multivalued, apply_filter, apply_random_sort, apply_sort, apply_str_filter,
    apply_time_filter,
};
use diesel::dsl::{InnerJoin, IntoBoxed, LeftJoin, Select, count, sql};
use diesel::expression::{SqlLiteral, UncheckedBind};
use diesel::pg::Pg;
use diesel::sql_types::{Float, SmallInt};
use diesel::{ExpressionMethods, JoinOnDsl, PgConnection, QueryDsl, QueryResult, RunQueryDsl};
use std::str::FromStr;
use strum::{EnumIter, EnumString, IntoStaticStr};

#[derive(Clone, Copy, EnumIter, EnumString, IntoStaticStr)]
#[strum(serialize_all = "kebab-case")]
#[strum(use_phf)]
pub enum Token {
    Id,
    FileSize,
    #[strum(serialize = "width", serialize = "image-width")]
    Width,
    #[strum(serialize = "height", serialize = "image-height")]
    Height,
    #[strum(serialize = "area", serialize = "image-area")]
    Area,
    #[strum(
        serialize = "ar",
        serialize = "aspect-ratio",
        serialize = "image-ar",
        serialize = "image-aspect-ratio"
    )]
    AspectRatio,
    #[strum(serialize = "rating", serialize = "safety")]
    Safety,
    Type,
    ContentChecksum,
    Flag,
    Source,
    Description,
    #[strum(
        serialize = "date",
        serialize = "time",
        serialize = "creation-date",
        serialize = "creation-time"
    )]
    CreationTime,
    #[strum(
        serialize = "edit-date",
        serialize = "edit-time",
        serialize = "last-edit-date",
        serialize = "last-edit-time"
    )]
    LastEditTime,
    Tag,
    Pool,
    #[strum(serialize = "submit", serialize = "upload", serialize = "uploader")]
    Uploader,
    Fav,
    Comment,
    NoteText,
    TagCount,
    CommentCount,
    RelationCount,
    NoteCount,
    FavCount,
    FeatureCount,
    Score,
    #[strum(serialize = "comment-date", serialize = "comment-time")]
    CommentTime,
    #[strum(serialize = "fav-date", serialize = "fav-time")]
    FavTime,
    #[strum(serialize = "feature-date", serialize = "feature-time")]
    FeatureTime,
    Special,
}

pub struct QueryBuilder<'a> {
    search: SearchCriteria<'a, Token>,
    cache: QueryCache,
}

impl<'a> Builder<'a> for QueryBuilder<'a> {
    type Token = Token;
    type BoxedQuery = BoxedQuery<'a>;

    fn criteria(&mut self) -> &mut SearchCriteria<'a, Self::Token> {
        &mut self.search
    }

    fn load(&mut self, conn: &mut PgConnection) -> ApiResult<Vec<i64>> {
        let query = self.build_filtered(conn)?;
        let query = self.apply_cache_filters(query);
        self.get_ordered_ids(conn, query).map_err(api::Error::from)
    }

    fn count(&mut self, conn: &mut PgConnection) -> ApiResult<i64> {
        if self.search.has_filter() {
            let unsorted_query = self.build_filtered(conn)?;
            let unsorted_query = self.apply_cache_filters(unsorted_query);
            unsorted_query.count().first(conn)
        } else {
            database_statistics::table
                .select(database_statistics::post_count)
                .first(conn)
        }
        .map_err(api::Error::from)
    }
}

impl<'a> QueryBuilder<'a> {
    pub fn new(client: Client, search_criteria: &'a str) -> ApiResult<Self> {
        let search = SearchCriteria::new(client, search_criteria, Token::Tag).map_err(Box::from)?;
        for sort in &search.sorts {
            match sort.kind {
                Token::ContentChecksum | Token::NoteText | Token::Special => return Err(api::Error::InvalidSort),
                _ => (),
            }
        }

        Ok(Self {
            search,
            cache: QueryCache::new(),
        })
    }

    fn build_filtered(&mut self, conn: &mut PgConnection) -> ApiResult<BoxedQuery<'a>> {
        let mut cache = self.cache.clone_if_empty();
        let base_query = post::table
            .select(post::id)
            .inner_join(post_statistics::table)
            .left_join(user::table)
            .into_boxed();
        let query = self
            .search
            .filters
            .iter()
            .try_fold(base_query, |query, &filter| match filter.kind {
                Token::Id => apply_filter!(query, post::id, filter, i64),
                Token::FileSize => apply_filter!(query, post::file_size, filter, i64),
                Token::Width => apply_filter!(query, post::width, filter, i32),
                Token::Height => apply_filter!(query, post::height, filter, i32),
                Token::Area => apply_filter!(query, post::width * post::height, filter, i32),
                Token::AspectRatio => apply_filter!(query, aspect_ratio(), filter, f32),
                Token::Safety => apply_filter!(query, post::safety, filter, PostSafety),
                Token::Type => apply_filter!(query, post::type_, filter, PostType),
                Token::ContentChecksum => apply_checksum_filter(query, filter),
                Token::Flag => apply_flag_filter(query, filter),
                Token::Source => Ok(apply_str_filter!(query, post::source, filter)),
                Token::Description => Ok(apply_str_filter!(query, post::description, filter)),
                Token::CreationTime => apply_time_filter!(query, post::creation_time, filter),
                Token::LastEditTime => apply_time_filter!(query, post::last_edit_time, filter),
                Token::Tag => apply_tag_filter(conn, query, filter, cache.as_mut()),
                Token::Pool => apply_pool_filter(conn, query, filter, cache.as_mut()),
                Token::Uploader => Ok(apply_str_filter!(query, user::name, filter)),
                Token::Fav => apply_favorite_filter(conn, query, filter, cache.as_mut()),
                Token::Comment => apply_comment_filter(conn, query, filter, cache.as_mut()),
                Token::NoteText => apply_note_text_filter(conn, query, filter, cache.as_mut()),
                Token::TagCount => apply_filter!(query, post_statistics::tag_count, filter, i64),
                Token::CommentCount => apply_filter!(query, post_statistics::comment_count, filter, i64),
                Token::RelationCount => apply_filter!(query, post_statistics::relation_count, filter, i64),
                Token::NoteCount => apply_filter!(query, post_statistics::note_count, filter, i64),
                Token::FavCount => apply_filter!(query, post_statistics::favorite_count, filter, i64),
                Token::FeatureCount => apply_filter!(query, post_statistics::feature_count, filter, i64),
                Token::Score => apply_filter!(query, post_statistics::score, filter, i64),
                Token::CommentTime => apply_comment_time_filter(conn, query, filter, cache.as_mut()),
                Token::FavTime => apply_favorite_time_filter(conn, query, filter, cache.as_mut()),
                Token::FeatureTime => apply_feature_time_filter(conn, query, filter, cache.as_mut()),
                Token::Special => apply_special_filter(conn, query, self.search.client, filter, cache.as_mut()),
            })?;
        self.cache.replace(cache);
        Ok(query)
    }

    fn get_ordered_ids(&self, conn: &mut PgConnection, unsorted_query: BoxedQuery<'a>) -> QueryResult<Vec<i64>> {
        // If random sort specified, no other sorts matter
        if self.search.random_sort {
            return apply_random_sort!(conn, self.search.client, unsorted_query, self.search).load(conn);
        }

        let default_sort = std::iter::once(ParsedSort {
            kind: Token::Id,
            order: Order::default(),
        });
        let sorts = self.search.sorts.iter().copied().chain(default_sort);
        let query = sorts.fold(unsorted_query, |query, sort| match sort.kind {
            Token::Id => apply_sort!(query, post::id, sort),
            Token::FileSize => apply_sort!(query, post::file_size, sort),
            Token::Width => apply_sort!(query, post::width, sort),
            Token::Height => apply_sort!(query, post::height, sort),
            Token::Area => apply_sort!(query, post::width * post::height, sort),
            Token::AspectRatio => apply_sort!(query, aspect_ratio(), sort),
            Token::Safety => apply_sort!(query, post::safety, sort),
            Token::Type => apply_sort!(query, post::type_, sort),
            Token::Flag => apply_sort!(query, post::flags, sort),
            Token::Source => apply_sort!(query, post::source, sort),
            Token::Description => apply_sort!(query, post::description, sort),
            Token::CreationTime => apply_sort!(query, post::creation_time, sort),
            Token::LastEditTime => apply_sort!(query, post::last_edit_time, sort),
            Token::Tag | Token::TagCount => apply_sort!(query, post_statistics::tag_count, sort),
            Token::Pool => apply_sort!(query, post_statistics::pool_count, sort),
            Token::Uploader => apply_sort!(query, user::name, sort),
            Token::Fav | Token::FavCount => apply_sort!(query, post_statistics::favorite_count, sort),
            Token::Comment | Token::CommentCount => apply_sort!(query, post_statistics::comment_count, sort),
            Token::RelationCount => apply_sort!(query, post_statistics::relation_count, sort),
            Token::NoteCount => apply_sort!(query, post_statistics::note_count, sort),
            Token::FeatureCount => apply_sort!(query, post_statistics::feature_count, sort),
            Token::Score => apply_sort!(query, post_statistics::score, sort),
            Token::CommentTime => apply_sort!(query, post_statistics::last_comment_time, sort),
            Token::FavTime => apply_sort!(query, post_statistics::last_favorite_time, sort),
            Token::FeatureTime => apply_sort!(query, post_statistics::last_feature_time, sort),
            Token::ContentChecksum | Token::NoteText | Token::Special => panic!("Invalid sort-style token!"),
        });
        match self.search.extra_args {
            Some(args) => query.offset(args.offset).limit(args.limit),
            None => query,
        }
        .load(conn)
    }

    fn apply_cache_filters(&'a self, mut query: BoxedQuery<'a>) -> BoxedQuery<'a> {
        if let Some(matching_ids) = self.cache.matches.as_ref() {
            query = query.filter(post::id.eq_any(matching_ids));
        }
        if let Some(nonmatching_ids) = self.cache.nonmatches.as_ref() {
            query = query.filter(post::id.ne_all(nonmatching_ids));
        }
        query
    }
}

type BoxedQuery<'a> =
    IntoBoxed<'a, LeftJoin<InnerJoin<Select<post::table, post::id>, post_statistics::table>, user::table>, Pg>;

#[derive(Clone, Copy, EnumString)]
#[strum(serialize_all = "kebab-case")]
enum SpecialToken {
    Liked,
    Disliked,
    Fav,
    Tumbleweed,
}

type Bind = UncheckedBind<SqlLiteral<Float, UncheckedBind<SqlLiteral<Float>, post::width>>, post::height>;

/// Returns a SQL literal representing a post's aspect. This is used instead of
/// `post::width / post::height` because it avoids integer truncation.
fn aspect_ratio() -> SqlLiteral<Float, Bind> {
    sql("CAST(")
        .bind(post::width)
        .sql(" AS REAL) / CAST(")
        .bind(post::height)
        .sql(" AS REAL)")
}

fn apply_checksum_filter<'a>(query: BoxedQuery<'a>, filter: UnparsedFilter<'a, Token>) -> ApiResult<BoxedQuery<'a>> {
    // Checksums can only be searched by exact value(s)
    let checksums: Vec<Checksum> = parse::values(filter.condition)?;
    Ok(if filter.negated {
        query.filter(post::checksum.ne_all(checksums))
    } else {
        query.filter(post::checksum.eq_any(checksums))
    })
}

fn apply_flag_filter<'a>(query: BoxedQuery<'a>, filter: UnparsedFilter<'a, Token>) -> ApiResult<BoxedQuery<'a>> {
    let flags: Vec<PostFlag> = parse::values(filter.condition)?;
    let value = flags.into_iter().fold(PostFlags::new(), |value, flag| value | flag);
    let bitwise_and = sql::<SmallInt>("")
        .bind(post::flags)
        .sql(" & ")
        .bind::<SmallInt, _>(value);
    Ok(if filter.negated {
        query.filter(bitwise_and.eq(0))
    } else {
        query.filter(bitwise_and.ne(0))
    })
}

fn apply_tag_filter<'a>(
    conn: &mut PgConnection,
    query: BoxedQuery<'a>,
    filter: UnparsedFilter<Token>,
    cache: Option<&mut QueryCache>,
) -> ApiResult<BoxedQuery<'a>> {
    if let Some(cache) = cache {
        let post_tags = post_tag::table
            .select(post_tag::post_id)
            .inner_join(tag_name::table.on(post_tag::tag_id.eq(tag_name::tag_id)))
            .into_boxed();
        let post_tags = apply_distinct_if_multivalued!(post_tags, filter);
        let filtered_posts = apply_str_filter!(post_tags, tag_name::name, filter.unnegated());
        let post_ids: Vec<i64> = filtered_posts.load(conn)?;
        cache.update(post_ids, filter.negated);
    }
    Ok(query)
}

fn apply_pool_filter<'a>(
    conn: &mut PgConnection,
    query: BoxedQuery<'a>,
    filter: UnparsedFilter<Token>,
    cache: Option<&mut QueryCache>,
) -> ApiResult<BoxedQuery<'a>> {
    if let Some(cache) = cache {
        let pool_posts = pool_post::table.select(pool_post::post_id).into_boxed();
        let pool_posts = apply_distinct_if_multivalued!(pool_posts, filter);
        let filtered_posts = apply_filter!(pool_posts, pool_post::pool_id, filter.unnegated(), i64)?;
        let post_ids: Vec<i64> = filtered_posts.load(conn)?;
        cache.update(post_ids, filter.negated);
    }
    Ok(query)
}

fn apply_favorite_filter<'a>(
    conn: &mut PgConnection,
    query: BoxedQuery<'a>,
    filter: UnparsedFilter<Token>,
    cache: Option<&mut QueryCache>,
) -> ApiResult<BoxedQuery<'a>> {
    if let Some(cache) = cache {
        let favorites = post_favorite::table
            .select(post_favorite::post_id)
            .inner_join(user::table)
            .into_boxed();
        let favorites = apply_distinct_if_multivalued!(favorites, filter);
        let filtered_posts = apply_str_filter!(favorites, user::name, filter.unnegated());
        let post_ids: Vec<i64> = filtered_posts.load(conn)?;
        cache.update(post_ids, filter.negated);
    }
    Ok(query)
}

fn apply_comment_filter<'a>(
    conn: &mut PgConnection,
    query: BoxedQuery<'a>,
    filter: UnparsedFilter<Token>,
    cache: Option<&mut QueryCache>,
) -> ApiResult<BoxedQuery<'a>> {
    if let Some(cache) = cache {
        let comments = comment::table
            .select(comment::post_id)
            .distinct()
            .inner_join(user::table)
            .into_boxed();
        let filtered_posts = apply_str_filter!(comments, user::name, filter.unnegated());
        let post_ids: Vec<i64> = filtered_posts.load(conn)?;
        cache.update(post_ids, filter.negated);
    }
    Ok(query)
}

fn apply_note_text_filter<'a>(
    conn: &mut PgConnection,
    query: BoxedQuery<'a>,
    filter: UnparsedFilter<Token>,
    cache: Option<&mut QueryCache>,
) -> ApiResult<BoxedQuery<'a>> {
    if let Some(cache) = cache {
        let post_notes = post_note::table.select(post_note::post_id).distinct().into_boxed();
        let filtered_posts = apply_str_filter!(post_notes, post_note::text, filter.unnegated());
        let post_ids: Vec<i64> = filtered_posts.load(conn)?;
        cache.update(post_ids, filter.negated);
    }
    Ok(query)
}

fn apply_comment_time_filter<'a>(
    conn: &mut PgConnection,
    query: BoxedQuery<'a>,
    filter: UnparsedFilter<Token>,
    cache: Option<&mut QueryCache>,
) -> ApiResult<BoxedQuery<'a>> {
    if let Some(cache) = cache {
        let comments = comment::table.select(comment::post_id).distinct().into_boxed();
        let filtered_posts = apply_time_filter!(comments, comment::creation_time, filter.unnegated())?;
        let post_ids: Vec<i64> = filtered_posts.load(conn)?;
        cache.update(post_ids, filter.negated);
    }
    Ok(query)
}

fn apply_favorite_time_filter<'a>(
    conn: &mut PgConnection,
    query: BoxedQuery<'a>,
    filter: UnparsedFilter<Token>,
    cache: Option<&mut QueryCache>,
) -> ApiResult<BoxedQuery<'a>> {
    if let Some(cache) = cache {
        let post_favorites = post_favorite::table
            .select(post_favorite::post_id)
            .distinct()
            .into_boxed();
        let filtered_posts = apply_time_filter!(post_favorites, post_favorite::time, filter.unnegated())?;
        let post_ids: Vec<i64> = filtered_posts.load(conn)?;
        cache.update(post_ids, filter.negated);
    }
    Ok(query)
}

fn apply_feature_time_filter<'a>(
    conn: &mut PgConnection,
    query: BoxedQuery<'a>,
    filter: UnparsedFilter<Token>,
    cache: Option<&mut QueryCache>,
) -> ApiResult<BoxedQuery<'a>> {
    if let Some(cache) = cache {
        let post_features = post_feature::table
            .select(post_feature::post_id)
            .distinct()
            .into_boxed();
        let filtered_posts = apply_time_filter!(post_features, post_feature::time, filter.unnegated())?;
        let post_ids: Vec<i64> = filtered_posts.load(conn)?;
        cache.update(post_ids, filter.negated);
    }
    Ok(query)
}

fn apply_special_filter<'a>(
    conn: &mut PgConnection,
    query: BoxedQuery<'a>,
    client: Client,
    filter: UnparsedFilter<Token>,
    cache: Option<&mut QueryCache>,
) -> ApiResult<BoxedQuery<'a>> {
    if let Some(cache) = cache {
        let special_token = SpecialToken::from_str(filter.condition).map_err(Box::from)?;
        let post_ids: Vec<i64> = match special_token {
            SpecialToken::Liked => client.id.ok_or(api::Error::NotLoggedIn).map(|client_id| {
                post_score::table
                    .select(post_score::post_id)
                    .filter(post_score::user_id.eq(client_id))
                    .filter(post_score::score.eq(1))
                    .load(conn)
            }),
            SpecialToken::Disliked => client.id.ok_or(api::Error::NotLoggedIn).map(|client_id| {
                post_score::table
                    .select(post_score::post_id)
                    .filter(post_score::user_id.eq(client_id))
                    .filter(post_score::score.eq(-1))
                    .load(conn)
            }),
            SpecialToken::Fav => client.id.ok_or(api::Error::NotLoggedIn).map(|client_id| {
                post_favorite::table
                    .select(post_favorite::post_id)
                    .filter(post_favorite::user_id.eq(client_id))
                    .load(conn)
            }),
            SpecialToken::Tumbleweed => {
                // A score of 0 doesn't necessarily mean no ratings, so we count them with a HAVING clause
                Ok(post_statistics::table
                    .select(post_statistics::post_id)
                    .left_join(post_score::table.on(post_score::post_id.eq(post_statistics::post_id)))
                    .filter(post_statistics::favorite_count.eq(0))
                    .filter(post_statistics::comment_count.eq(0))
                    .group_by(post_statistics::post_id)
                    .having(count(post_score::post_id).eq(0))
                    .load(conn))
            }
        }??;
        cache.update(post_ids, filter.negated);
    }
    Ok(query)
}
