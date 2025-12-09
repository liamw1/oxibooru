use crate::api::error::{ApiError, ApiResult};
use crate::auth::Client;
use crate::content::hash::Checksum;
use crate::model::enums::{PostFlag, PostFlags, PostSafety, PostType};
use crate::schema::{
    comment, database_statistics, pool_post, post, post_favorite, post_feature, post_note, post_score, post_statistics,
    post_tag, tag_name, user,
};
use crate::search::{
    self, Builder, CacheState, Condition, Order, ParsedSort, SearchCriteria, StrCondition, UnparsedFilter, parse,
};
use crate::{
    apply_cache_filters, apply_distinct_if_multivalued, apply_filter, apply_random_sort, apply_sort, apply_str_filter,
    apply_time_filter, update_filter_cache, update_nonmatching_filter_cache,
};
use diesel::dsl::{Eq, InnerJoin, InnerJoinOn, IntoBoxed, LeftJoin, Select, count, sql};
use diesel::expression::{SqlLiteral, UncheckedBind};
use diesel::pg::Pg;
use diesel::sql_types::{Float, SmallInt};
use diesel::{ExpressionMethods, JoinOnDsl, PgConnection, QueryDsl, QueryResult, RunQueryDsl, TextExpressionMethods};
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
    cache_state: CacheState,
}

impl<'a> Builder<'a> for QueryBuilder<'a> {
    type Token = Token;
    type BoxedQuery = BoxedQuery;

    fn new(client: Client, search_criteria: &'a str) -> ApiResult<Self> {
        let search = SearchCriteria::new(client, search_criteria, Token::Tag).map_err(Box::from)?;
        for sort in &search.sorts {
            match sort.kind {
                Token::ContentChecksum | Token::NoteText | Token::Special => return Err(ApiError::InvalidSort),
                _ => (),
            }
        }

        Ok(Self {
            search,
            cache_state: CacheState::new(),
        })
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
                .select(database_statistics::post_count)
                .first(conn)
        }
        .map_err(ApiError::from)
    }

    fn build_filtered(&mut self, conn: &mut PgConnection) -> ApiResult<BoxedQuery> {
        let mut nonmatching_posts = None;
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
                Token::Tag => apply_tag_filter(conn, query, &mut nonmatching_posts, filter, &mut self.cache_state),
                Token::Pool => apply_pool_filter(conn, query, filter, &mut self.cache_state),
                Token::Uploader => Ok(apply_str_filter!(query, user::name, filter)),
                Token::Fav => apply_favorite_filter(conn, query, filter, &mut self.cache_state),
                Token::Comment => apply_comment_filter(conn, query, filter, &mut self.cache_state),
                Token::NoteText => apply_note_text_filter(conn, query, filter, &mut self.cache_state),
                Token::TagCount => apply_filter!(query, post_statistics::tag_count, filter, i64),
                Token::CommentCount => apply_filter!(query, post_statistics::comment_count, filter, i64),
                Token::RelationCount => apply_filter!(query, post_statistics::relation_count, filter, i64),
                Token::NoteCount => apply_filter!(query, post_statistics::note_count, filter, i64),
                Token::FavCount => apply_filter!(query, post_statistics::favorite_count, filter, i64),
                Token::FeatureCount => apply_filter!(query, post_statistics::feature_count, filter, i64),
                Token::Score => apply_filter!(query, post_statistics::score, filter, i64),
                Token::CommentTime => apply_comment_time_filter(conn, query, filter, &mut self.cache_state),
                Token::FavTime => apply_favorite_time_filter(conn, query, filter, &mut self.cache_state),
                Token::FeatureTime => apply_feature_time_filter(conn, query, filter, &mut self.cache_state),
                Token::Special => apply_special_filter(conn, query, self.search.client, filter, &mut self.cache_state),
            })?;
        if let Some(nonmatching) = nonmatching_posts {
            update_nonmatching_filter_cache!(conn, nonmatching, self.cache_state)?;
        }
        Ok(apply_cache_filters!(query, post::id, self.cache_state))
    }

    fn get_ordered_ids(&self, conn: &mut PgConnection, unsorted_query: BoxedQuery) -> QueryResult<Vec<i64>> {
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
}

type BoxedQuery =
    IntoBoxed<'static, LeftJoin<InnerJoin<Select<post::table, post::id>, post_statistics::table>, user::table>, Pg>;

type NonmatchingPostTags = IntoBoxed<
    'static,
    InnerJoinOn<Select<post_tag::table, post_tag::post_id>, tag_name::table, Eq<post_tag::tag_id, tag_name::tag_id>>,
    Pg,
>;

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

fn apply_checksum_filter(query: BoxedQuery, filter: UnparsedFilter<Token>) -> ApiResult<BoxedQuery> {
    // Checksums can only be searched by exact value(s)
    let checksums: Vec<Checksum> = parse::values(filter.condition)?;
    Ok(if filter.negated {
        query.filter(post::checksum.ne_all(checksums))
    } else {
        query.filter(post::checksum.eq_any(checksums))
    })
}

fn apply_flag_filter(query: BoxedQuery, filter: UnparsedFilter<Token>) -> ApiResult<BoxedQuery> {
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

fn apply_tag_filter(
    conn: &mut PgConnection,
    query: BoxedQuery,
    nonmatching_posts: &mut Option<NonmatchingPostTags>,
    filter: UnparsedFilter<Token>,
    state: &mut CacheState,
) -> ApiResult<BoxedQuery> {
    let post_tags = post_tag::table
        .select(post_tag::post_id)
        .inner_join(tag_name::table.on(post_tag::tag_id.eq(tag_name::tag_id)))
        .into_boxed();

    if filter.negated {
        // We can perform an optimization here for negative tag filters. Instead of querying to insert nonmatching
        // posts into the TEMP table one filter at a time, we can union together the filters to produce a single
        // query to retrieve all posts that are excluded by the negative filters.
        let nonmatching = nonmatching_posts.take().unwrap_or(post_tags.distinct());
        let nonmatching = match parse::str_condition(filter.condition) {
            StrCondition::Regular(Condition::Values(values)) => nonmatching.or_filter(tag_name::name.eq_any(values)),
            StrCondition::Regular(Condition::GreaterEq(value)) => nonmatching.or_filter(tag_name::name.ge(value)),
            StrCondition::Regular(Condition::LessEq(value)) => nonmatching.or_filter(tag_name::name.le(value)),
            StrCondition::Regular(Condition::Range(range)) => {
                nonmatching.or_filter(tag_name::name.between(range.start, range.end))
            }
            StrCondition::WildCard(pattern) => nonmatching.or_filter(search::lower(tag_name::name).like(pattern)),
        };
        nonmatching_posts.replace(nonmatching);
    } else {
        let post_tags = apply_distinct_if_multivalued!(post_tags, filter);
        let filtered_posts = apply_str_filter!(post_tags, tag_name::name, filter.unnegated());
        update_filter_cache!(conn, filtered_posts, post_tag::post_id, filter, state)?;
    }
    Ok(query)
}

fn apply_pool_filter(
    conn: &mut PgConnection,
    query: BoxedQuery,
    filter: UnparsedFilter<Token>,
    state: &mut CacheState,
) -> ApiResult<BoxedQuery> {
    let pool_posts = pool_post::table.select(pool_post::post_id).into_boxed();
    let pool_posts = apply_distinct_if_multivalued!(pool_posts, filter);
    let filtered_posts = apply_filter!(pool_posts, pool_post::pool_id, filter.unnegated(), i64)?;
    update_filter_cache!(conn, filtered_posts, pool_post::post_id, filter, state)?;
    Ok(query)
}

fn apply_favorite_filter(
    conn: &mut PgConnection,
    query: BoxedQuery,
    filter: UnparsedFilter<Token>,
    state: &mut CacheState,
) -> ApiResult<BoxedQuery> {
    let favorites = post_favorite::table
        .select(post_favorite::post_id)
        .inner_join(user::table)
        .into_boxed();
    let favorites = apply_distinct_if_multivalued!(favorites, filter);
    let filtered_posts = apply_str_filter!(favorites, user::name, filter.unnegated());
    update_filter_cache!(conn, filtered_posts, post_favorite::post_id, filter, state)?;
    Ok(query)
}

fn apply_comment_filter(
    conn: &mut PgConnection,
    query: BoxedQuery,
    filter: UnparsedFilter<Token>,
    state: &mut CacheState,
) -> ApiResult<BoxedQuery> {
    let comments = comment::table
        .select(comment::post_id)
        .distinct()
        .inner_join(user::table)
        .into_boxed();
    let filtered_posts = apply_str_filter!(comments, user::name, filter.unnegated());
    update_filter_cache!(conn, filtered_posts, comment::post_id, filter, state)?;
    Ok(query)
}

fn apply_note_text_filter(
    conn: &mut PgConnection,
    query: BoxedQuery,
    filter: UnparsedFilter<Token>,
    state: &mut CacheState,
) -> ApiResult<BoxedQuery> {
    let post_notes = post_note::table.select(post_note::post_id).distinct().into_boxed();
    let filtered_posts = apply_str_filter!(post_notes, post_note::text, filter.unnegated());
    update_filter_cache!(conn, filtered_posts, post_note::post_id, filter, state)?;
    Ok(query)
}

fn apply_comment_time_filter(
    conn: &mut PgConnection,
    query: BoxedQuery,
    filter: UnparsedFilter<Token>,
    state: &mut CacheState,
) -> ApiResult<BoxedQuery> {
    let comments = comment::table.select(comment::post_id).distinct().into_boxed();
    let filtered_posts = apply_time_filter!(comments, comment::creation_time, filter.unnegated())?;
    update_filter_cache!(conn, filtered_posts, comment::post_id, filter, state)?;
    Ok(query)
}

fn apply_favorite_time_filter(
    conn: &mut PgConnection,
    query: BoxedQuery,
    filter: UnparsedFilter<Token>,
    state: &mut CacheState,
) -> ApiResult<BoxedQuery> {
    let post_favorites = post_favorite::table
        .select(post_favorite::post_id)
        .distinct()
        .into_boxed();
    let filtered_posts = apply_time_filter!(post_favorites, post_favorite::time, filter.unnegated())?;
    update_filter_cache!(conn, filtered_posts, post_favorite::post_id, filter, state)?;
    Ok(query)
}

fn apply_feature_time_filter(
    conn: &mut PgConnection,
    query: BoxedQuery,
    filter: UnparsedFilter<Token>,
    state: &mut CacheState,
) -> ApiResult<BoxedQuery> {
    let post_features = post_feature::table
        .select(post_feature::post_id)
        .distinct()
        .into_boxed();
    let filtered_posts = apply_time_filter!(post_features, post_feature::time, filter.unnegated())?;
    update_filter_cache!(conn, filtered_posts, post_feature::post_id, filter, state)?;
    Ok(query)
}

fn apply_special_filter(
    conn: &mut PgConnection,
    query: BoxedQuery,
    client: Client,
    filter: UnparsedFilter<Token>,
    state: &mut CacheState,
) -> ApiResult<BoxedQuery> {
    let special_token = SpecialToken::from_str(filter.condition).map_err(Box::from)?;
    match special_token {
        SpecialToken::Liked => client.id.ok_or(ApiError::NotLoggedIn).map(|client_id| {
            let filtered_posts = post_score::table
                .select(post_score::post_id)
                .filter(post_score::user_id.eq(client_id))
                .filter(post_score::score.eq(1));
            update_filter_cache!(conn, filtered_posts, post_score::post_id, filter, state)
        }),
        SpecialToken::Disliked => client.id.ok_or(ApiError::NotLoggedIn).map(|client_id| {
            let filtered_posts = post_score::table
                .select(post_score::post_id)
                .filter(post_score::user_id.eq(client_id))
                .filter(post_score::score.eq(-1));
            update_filter_cache!(conn, filtered_posts, post_score::post_id, filter, state)
        }),
        SpecialToken::Fav => client.id.ok_or(ApiError::NotLoggedIn).map(|client_id| {
            let filtered_posts = post_favorite::table
                .select(post_favorite::post_id)
                .filter(post_favorite::user_id.eq(client_id));
            update_filter_cache!(conn, filtered_posts, post_favorite::post_id, filter, state)
        }),
        SpecialToken::Tumbleweed => {
            // A score of 0 doesn't necessarily mean no ratings, so we count them with a HAVING clause
            let filtered_posts = post_statistics::table
                .select(post_statistics::post_id)
                .left_join(post_score::table.on(post_score::post_id.eq(post_statistics::post_id)))
                .filter(post_statistics::favorite_count.eq(0))
                .filter(post_statistics::comment_count.eq(0))
                .group_by(post_statistics::post_id)
                .having(count(post_score::post_id).eq(0));
            Ok(update_filter_cache!(conn, filtered_posts, post_statistics::post_id, filter, state))
        }
    }??;
    Ok(query)
}
