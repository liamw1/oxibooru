use crate::model::enums::{PostSafety, PostType};
use crate::schema::{
    comment, pool_post, post, post_favorite, post_feature, post_note, post_relation, post_score, post_tag, tag_name,
    user,
};
use crate::search::{Error, Order, ParsedSort, QueryArgs, SearchCriteria};
use crate::{apply_filter, apply_having_clause, apply_str_filter, apply_time_filter, finalize};
use diesel::dsl::*;
use diesel::pg::Pg;
use diesel::prelude::*;
use strum::EnumString;

pub type BoxedQuery<'a> = IntoBoxed<'a, Select<post::table, post::id>, Pg>;

#[derive(Clone, Copy, EnumString)]
#[strum(serialize_all = "kebab-case")]
#[strum(use_phf)]
pub enum Token {
    Id,
    FileSize,
    #[strum(serialize = "width", serialize = "image-width")]
    ImageWidth,
    #[strum(serialize = "height", serialize = "image-height")]
    ImageHeight,
    #[strum(serialize = "area", serialize = "image-area")]
    ImageArea,
    #[strum(
        serialize = "ar",
        serialize = "aspect-ratio",
        serialize = "image-ar",
        serialize = "image-aspect-ratio"
    )]
    ImageAspectRatio,
    #[strum(serialize = "rating", serialize = "safety")]
    Safety,
    Type,
    ContentChecksum,
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

    // Requires join
    Tag,
    #[strum(serialize = "submit", serialize = "upload", serialize = "uploader")]
    Uploader,
    Pool,
    NoteText,
    TagCount,
    CommentCount,
    RelationCount,
    NoteCount,
    FavCount,
    FeatureCount,
    #[strum(serialize = "comment-date", serialize = "comment-time")]
    CommentTime,
    #[strum(serialize = "fav-date", serialize = "fav-time")]
    FavTime,
    #[strum(serialize = "feature-date", serialize = "feature-time")]
    FeatureTime,
}

pub fn parse_search_criteria(search_criteria: &str) -> Result<SearchCriteria<Token>, Error> {
    SearchCriteria::new(search_criteria, Token::Id)
        .map_err(Box::from)
        .map_err(Error::from)
}

pub fn build_query<'a>(
    client: Option<i32>,
    search_criteria: &'a SearchCriteria<Token>,
) -> Result<BoxedQuery<'a>, Error> {
    let base_query = post::table.select(post::id).into_boxed();
    let query = search_criteria
        .filters
        .iter()
        .try_fold(base_query, |query, filter| match filter.kind {
            Token::Id => apply_filter!(query, post::id, filter, i32),
            Token::FileSize => apply_filter!(query, post::file_size, filter, i64),
            Token::ImageWidth => apply_filter!(query, post::width, filter, i32),
            Token::ImageHeight => apply_filter!(query, post::height, filter, i32),
            Token::ImageArea => apply_filter!(query, post::width * post::height, filter, i32),
            Token::ImageAspectRatio => apply_filter!(query, post::width / post::height, filter, i32),
            Token::Safety => apply_filter!(query, post::safety, filter, PostSafety),
            Token::Type => apply_filter!(query, post::type_, filter, PostType),
            Token::ContentChecksum => Ok(apply_str_filter!(query, post::checksum, filter)),
            Token::CreationTime => apply_time_filter!(query, post::creation_time, filter),
            Token::LastEditTime => apply_time_filter!(query, post::last_edit_time, filter),
            Token::Tag => {
                let tags = post_tag::table
                    .select(post_tag::post_id)
                    .inner_join(tag_name::table.on(post_tag::tag_id.eq(tag_name::tag_id)))
                    .into_boxed();
                let subquery = apply_str_filter!(tags, tag_name::name, filter);
                Ok(query.filter(post::id.eq_any(subquery)))
            }
            Token::Uploader => {
                let users = user::table.select(user::id).into_boxed();
                let subquery = apply_str_filter!(users, user::name, filter);
                Ok(query.filter(post::user_id.eq_any(subquery.nullable())))
            }
            Token::Pool => {
                let pool_posts = pool_post::table.select(pool_post::post_id).into_boxed();
                apply_filter!(pool_posts, pool_post::pool_id, filter, i32)
                    .map(|subquery| query.filter(post::id.eq_any(subquery)))
            }
            Token::NoteText => {
                let post_notes = post_note::table.select(post_note::post_id).into_boxed();
                let subquery = apply_str_filter!(post_notes, post_note::text, filter);
                Ok(query.filter(post::id.eq_any(subquery)))
            }
            Token::TagCount => {
                let post_tags = post::table
                    .select(post::id)
                    .left_join(post_tag::table)
                    .group_by(post::id)
                    .into_boxed();
                apply_having_clause!(post_tags, count(post_tag::tag_id), filter)
                    .map(|subquery| query.filter(post::id.eq_any(subquery)))
            }
            Token::CommentCount => {
                let comments = post::table
                    .select(post::id)
                    .left_join(comment::table)
                    .group_by(post::id)
                    .into_boxed();
                apply_having_clause!(comments, count(comment::id), filter)
                    .map(|subquery| query.filter(post::id.eq_any(subquery)))
            }
            Token::RelationCount => {
                let post_relations = post::table
                    .select(post::id)
                    .left_join(post_relation::table)
                    .group_by(post::id)
                    .into_boxed();
                apply_having_clause!(post_relations, count(post_relation::child_id), filter)
                    .map(|subquery| query.filter(post::id.eq_any(subquery)))
            }
            Token::NoteCount => {
                let post_notes = post::table
                    .select(post::id)
                    .left_join(post_note::table)
                    .group_by(post::id)
                    .into_boxed();
                apply_having_clause!(post_notes, count(post_note::id), filter)
                    .map(|subquery| query.filter(post::id.eq_any(subquery)))
            }
            Token::FavCount => {
                let post_favorites = post::table
                    .select(post::id)
                    .left_join(post_favorite::table)
                    .group_by(post::id)
                    .into_boxed();
                apply_having_clause!(post_favorites, count(post_favorite::user_id), filter)
                    .map(|subquery| query.filter(post::id.eq_any(subquery)))
            }
            Token::FeatureCount => {
                let post_features = post::table
                    .select(post::id)
                    .left_join(post_feature::table)
                    .group_by(post::id)
                    .into_boxed();
                apply_having_clause!(post_features, count(post_feature::id), filter)
                    .map(|subquery| query.filter(post::id.eq_any(subquery)))
            }
            Token::CommentTime => {
                let comments = comment::table.select(comment::post_id).into_boxed();
                apply_time_filter!(comments, comment::creation_time, filter)
                    .map(|subquery| query.filter(post::id.eq_any(subquery)))
            }
            Token::FavTime => {
                let post_favorites = post_favorite::table.select(post_favorite::post_id).into_boxed();
                apply_time_filter!(post_favorites, post_favorite::time, filter)
                    .map(|subquery| query.filter(post::id.eq_any(subquery)))
            }
            Token::FeatureTime => {
                let post_features = post_feature::table.select(post_feature::post_id).into_boxed();
                apply_time_filter!(post_features, post_feature::time, filter)
                    .map(|subquery| query.filter(post::id.eq_any(subquery)))
            }
        })?;

    let special_tokens = search_criteria.parse_special_tokens().map_err(Box::from)?;
    special_tokens.iter().try_fold(query, |query, token| match token {
        SpecialToken::Liked => client.ok_or(Error::NotLoggedIn).map(|client_id| {
            let subquery = post_score::table
                .select(post_score::post_id)
                .filter(post_score::user_id.eq(client_id))
                .filter(post_score::score.eq(1));
            query.filter(post::id.eq_any(subquery))
        }),
        SpecialToken::Disliked => client.ok_or(Error::NotLoggedIn).map(|client_id| {
            let subquery = post_score::table
                .select(post_score::post_id)
                .filter(post_score::user_id.eq(client_id))
                .filter(post_score::score.eq(-1));
            query.filter(post::id.eq_any(subquery))
        }),
        SpecialToken::Fav => client.ok_or(Error::NotLoggedIn).map(|client_id| {
            let subquery = post_favorite::table
                .select(post_favorite::post_id)
                .filter(post_favorite::user_id.eq(client_id));
            query.filter(post::id.eq_any(subquery))
        }),
        SpecialToken::Tumbleweed => {
            // I'm not sure why these need to be boxed
            let score_subquery = post::table
                .select(post::id)
                .left_join(post_score::table)
                .group_by(post::id)
                .having(count(post_score::post_id).eq(0))
                .into_boxed();
            let favorite_subquery = post::table
                .select(post::id)
                .left_join(post_favorite::table)
                .group_by(post::id)
                .having(count(post_favorite::post_id).eq(0))
                .into_boxed();
            let comment_subquery = post::table
                .select(post::id)
                .left_join(comment::table)
                .group_by(post::id)
                .having(count(comment::post_id).eq(0))
                .into_boxed();

            Ok(query
                .filter(post::id.eq_any(score_subquery))
                .filter(post::id.eq_any(favorite_subquery))
                .filter(post::id.eq_any(comment_subquery)))
        }
    })
}

pub fn get_ordered_ids(
    conn: &mut PgConnection,
    query: BoxedQuery,
    search_criteria: &SearchCriteria<Token>,
) -> QueryResult<Vec<i32>> {
    // If random sort specified, no other sorts matter
    let extra_args = search_criteria.extra_args;
    if search_criteria.random_sort {
        define_sql_function!(fn random() -> Integer);
        return match extra_args {
            Some(args) => query.order(random()).offset(args.offset).limit(args.limit),
            None => query.order(random()),
        }
        .load(conn);
    }

    // Add default sort if none specified
    let sort = search_criteria.sorts.last().cloned().unwrap_or(ParsedSort {
        kind: Token::Id,
        order: Order::default(),
    });

    match sort.kind {
        Token::Id => finalize!(query, post::id, sort, extra_args).load(conn),
        Token::FileSize => finalize!(query, post::file_size, sort, extra_args).load(conn),
        Token::ImageWidth => finalize!(query, post::width, sort, extra_args).load(conn),
        Token::ImageHeight => finalize!(query, post::height, sort, extra_args).load(conn),
        Token::ImageArea => finalize!(query, post::width * post::height, sort, extra_args).load(conn),
        Token::ImageAspectRatio => finalize!(query, post::width / post::height, sort, extra_args).load(conn),
        Token::Safety => finalize!(query, post::safety, sort, extra_args).load(conn),
        Token::Type => finalize!(query, post::type_, sort, extra_args).load(conn),
        Token::ContentChecksum => finalize!(query, post::checksum, sort, extra_args).load(conn),
        Token::CreationTime => finalize!(query, post::creation_time, sort, extra_args).load(conn),
        Token::LastEditTime => finalize!(query, post::last_edit_time, sort, extra_args).load(conn),

        /*
            The implementation for these isn't ideal, but it's the best thing I could do given
            Diesel's annoying restrictions around dynamic queries. If you could call .grouped_by
            on a boxed query, the implementation could be so much nicer.
        */
        Token::Tag => tag_count_sorted(conn, query, sort, extra_args),
        Token::Uploader => uploader_sorted(conn, query, sort, extra_args),
        Token::Pool => pool_sorted(conn, query, sort, extra_args),
        Token::NoteText => note_text_sorted(conn, query, sort, extra_args),
        Token::TagCount => tag_count_sorted(conn, query, sort, extra_args),
        Token::CommentCount => comment_count_sorted(conn, query, sort, extra_args),
        Token::RelationCount => relation_count_sorted(conn, query, sort, extra_args),
        Token::NoteCount => note_count_sorted(conn, query, sort, extra_args),
        Token::FavCount => favorite_count_sorted(conn, query, sort, extra_args),
        Token::FeatureCount => feature_count_sorted(conn, query, sort, extra_args),
        Token::CommentTime => comment_time_sorted(conn, query, sort, extra_args),
        Token::FavTime => favorite_time_sorted(conn, query, sort, extra_args),
        Token::FeatureTime => feature_time_sorted(conn, query, sort, extra_args),
    }
}

#[derive(Clone, Copy, EnumString)]
#[strum(serialize_all = "kebab-case")]
enum SpecialToken {
    Liked,
    Disliked,
    Fav,
    Tumbleweed,
}

fn uploader_sorted(
    conn: &mut PgConnection,
    query: BoxedQuery,
    sort: ParsedSort<Token>,
    extra_args: Option<QueryArgs>,
) -> QueryResult<Vec<i32>> {
    let filtered_posts: Vec<i32> = query.load(conn)?;
    let final_query = post::table
        .select(post::id)
        .group_by(post::id)
        .left_join(user::table)
        .filter(post::id.eq_any(&filtered_posts))
        .into_boxed();
    finalize!(final_query, min(user::name), sort, extra_args).load(conn)
}

fn pool_sorted(
    conn: &mut PgConnection,
    query: BoxedQuery,
    sort: ParsedSort<Token>,
    extra_args: Option<QueryArgs>,
) -> QueryResult<Vec<i32>> {
    let filtered_posts: Vec<i32> = query.load(conn)?;
    let final_query = post::table
        .select(post::id)
        .group_by(post::id)
        .left_join(pool_post::table)
        .filter(post::id.eq_any(&filtered_posts))
        .into_boxed();
    finalize!(final_query, count(pool_post::pool_id), sort, extra_args).load(conn)
}

fn note_text_sorted(
    conn: &mut PgConnection,
    query: BoxedQuery,
    sort: ParsedSort<Token>,
    extra_args: Option<QueryArgs>,
) -> QueryResult<Vec<i32>> {
    let filtered_posts: Vec<i32> = query.load(conn)?;
    let final_query = post::table
        .select(post::id)
        .group_by(post::id)
        .left_join(post_note::table)
        .filter(post::id.eq_any(&filtered_posts))
        .into_boxed();
    finalize!(final_query, min(post_note::text), sort, extra_args).load(conn)
}

fn tag_count_sorted(
    conn: &mut PgConnection,
    query: BoxedQuery,
    sort: ParsedSort<Token>,
    extra_args: Option<QueryArgs>,
) -> QueryResult<Vec<i32>> {
    let filtered_posts: Vec<i32> = query.load(conn)?;
    let final_query = post::table
        .select(post::id)
        .group_by(post::id)
        .left_join(post_tag::table)
        .filter(post::id.eq_any(&filtered_posts))
        .into_boxed();
    finalize!(final_query, count(post_tag::tag_id), sort, extra_args).load(conn)
}

fn comment_count_sorted(
    conn: &mut PgConnection,
    query: BoxedQuery,
    sort: ParsedSort<Token>,
    extra_args: Option<QueryArgs>,
) -> QueryResult<Vec<i32>> {
    let filtered_posts: Vec<i32> = query.load(conn)?;
    let final_query = post::table
        .select(post::id)
        .group_by(post::id)
        .left_join(comment::table)
        .filter(post::id.eq_any(&filtered_posts))
        .into_boxed();
    finalize!(final_query, count(comment::id), sort, extra_args).load(conn)
}

fn relation_count_sorted(
    conn: &mut PgConnection,
    query: BoxedQuery,
    sort: ParsedSort<Token>,
    extra_args: Option<QueryArgs>,
) -> QueryResult<Vec<i32>> {
    let filtered_posts: Vec<i32> = query.load(conn)?;
    let final_query = post::table
        .select(post::id)
        .group_by(post::id)
        .left_join(post_relation::table)
        .filter(post::id.eq_any(&filtered_posts))
        .into_boxed();
    finalize!(final_query, count(post_relation::child_id), sort, extra_args).load(conn)
}

fn note_count_sorted(
    conn: &mut PgConnection,
    query: BoxedQuery,
    sort: ParsedSort<Token>,
    extra_args: Option<QueryArgs>,
) -> QueryResult<Vec<i32>> {
    let filtered_posts: Vec<i32> = query.load(conn)?;
    let final_query = post::table
        .select(post::id)
        .group_by(post::id)
        .left_join(post_note::table)
        .filter(post::id.eq_any(&filtered_posts))
        .into_boxed();
    finalize!(final_query, count(post_note::id), sort, extra_args).load(conn)
}

fn favorite_count_sorted(
    conn: &mut PgConnection,
    query: BoxedQuery,
    sort: ParsedSort<Token>,
    extra_args: Option<QueryArgs>,
) -> QueryResult<Vec<i32>> {
    let filtered_posts: Vec<i32> = query.load(conn)?;
    let final_query = post::table
        .select(post::id)
        .group_by(post::id)
        .left_join(post_favorite::table)
        .filter(post::id.eq_any(&filtered_posts))
        .into_boxed();
    finalize!(final_query, count(post_favorite::user_id), sort, extra_args).load(conn)
}

fn feature_count_sorted(
    conn: &mut PgConnection,
    query: BoxedQuery,
    sort: ParsedSort<Token>,
    extra_args: Option<QueryArgs>,
) -> QueryResult<Vec<i32>> {
    let filtered_posts: Vec<i32> = query.load(conn)?;
    let final_query = post::table
        .select(post::id)
        .group_by(post::id)
        .left_join(post_feature::table)
        .filter(post::id.eq_any(&filtered_posts))
        .into_boxed();
    finalize!(final_query, count(post_feature::id), sort, extra_args).load(conn)
}

fn comment_time_sorted(
    conn: &mut PgConnection,
    query: BoxedQuery,
    sort: ParsedSort<Token>,
    extra_args: Option<QueryArgs>,
) -> QueryResult<Vec<i32>> {
    let filtered_posts: Vec<i32> = query.load(conn)?;
    let final_query = post::table
        .select(post::id)
        .group_by(post::id)
        .left_join(comment::table)
        .filter(post::id.eq_any(&filtered_posts))
        .into_boxed();
    finalize!(final_query, max(comment::creation_time), sort, extra_args).load(conn)
}

fn favorite_time_sorted(
    conn: &mut PgConnection,
    query: BoxedQuery,
    sort: ParsedSort<Token>,
    extra_args: Option<QueryArgs>,
) -> QueryResult<Vec<i32>> {
    let filtered_posts: Vec<i32> = query.load(conn)?;
    let final_query = post::table
        .select(post::id)
        .group_by(post::id)
        .left_join(post_favorite::table)
        .filter(post::id.eq_any(&filtered_posts))
        .into_boxed();
    finalize!(final_query, max(post_favorite::time), sort, extra_args).load(conn)
}

fn feature_time_sorted(
    conn: &mut PgConnection,
    query: BoxedQuery,
    sort: ParsedSort<Token>,
    extra_args: Option<QueryArgs>,
) -> QueryResult<Vec<i32>> {
    let filtered_posts: Vec<i32> = query.load(conn)?;
    let final_query = post::table
        .select(post::id)
        .group_by(post::id)
        .left_join(post_feature::table)
        .filter(post::id.eq_any(&filtered_posts))
        .into_boxed();
    finalize!(final_query, max(post_feature::time), sort, extra_args).load(conn)
}
