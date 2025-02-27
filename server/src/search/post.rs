use crate::auth::header::Client;
use crate::model::enums::{PostSafety, PostType};
use crate::schema::{
    comment, pool_post, post, post_favorite, post_feature, post_note, post_score, post_statistics, post_tag, tag_name,
    user,
};
use crate::search::{Error, Order, ParsedSort, SearchCriteria, UnparsedFilter};
use crate::{apply_filter, apply_sort, apply_str_filter, apply_subquery_filter, apply_time_filter};
use diesel::dsl::{count, sql, InnerJoin, IntoBoxed, LeftJoin, Select};
use diesel::expression::{SqlLiteral, UncheckedBind};
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::sql_types::Float;
use std::str::FromStr;
use strum::{EnumIter, EnumString, IntoStaticStr};

pub type BoxedQuery<'a> =
    IntoBoxed<'a, LeftJoin<InnerJoin<Select<post::table, post::id>, post_statistics::table>, user::table>, Pg>;

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
    #[strum(serialize = "comment-date", serialize = "comment-time")]
    CommentTime,
    #[strum(serialize = "fav-date", serialize = "fav-time")]
    FavTime,
    #[strum(serialize = "feature-date", serialize = "feature-time")]
    FeatureTime,
    Special,
}

pub fn parse_search_criteria(search_criteria: &str) -> Result<SearchCriteria<Token>, Error> {
    let criteria = SearchCriteria::new(search_criteria, Token::Tag).map_err(Box::from)?;
    for sort in criteria.sorts.iter() {
        match sort.kind {
            Token::ContentChecksum | Token::NoteText | Token::Special => return Err(Error::InvalidSort),
            _ => (),
        }
    }
    Ok(criteria)
}

pub fn build_query<'a>(client: Client, search_criteria: &'a SearchCriteria<Token>) -> Result<BoxedQuery<'a>, Error> {
    let base_query = post::table
        .select(post::id)
        .inner_join(post_statistics::table)
        .left_join(user::table)
        .into_boxed();
    search_criteria
        .filters
        .iter()
        .try_fold(base_query, |query, filter| match filter.kind {
            Token::Id => apply_filter!(query, post::id, filter, i64),
            Token::FileSize => apply_filter!(query, post::file_size, filter, i64),
            Token::Width => apply_filter!(query, post::width, filter, i32),
            Token::Height => apply_filter!(query, post::height, filter, i32),
            Token::Area => apply_filter!(query, post::width * post::height, filter, i32),
            Token::AspectRatio => apply_filter!(query, aspect_ratio(), filter, f32),
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
                let subquery = apply_str_filter!(tags, tag_name::name, filter.unnegated());
                Ok(apply_subquery_filter!(query, post::id, filter, subquery))
            }
            Token::Pool => {
                let pool_posts = pool_post::table.select(pool_post::post_id).into_boxed();
                let subquery = apply_filter!(pool_posts, pool_post::pool_id, filter.unnegated(), i64)?;
                Ok(apply_subquery_filter!(query, post::id, filter, subquery))
            }
            Token::Uploader => Ok(apply_str_filter!(query, user::name, filter)),
            Token::Fav => {
                let favorites = post_favorite::table
                    .select(post_favorite::post_id)
                    .inner_join(user::table)
                    .into_boxed();
                let subquery = apply_str_filter!(favorites, user::name, filter.unnegated());
                Ok(apply_subquery_filter!(query, post::id, filter, subquery))
            }
            Token::Comment => {
                let comments = comment::table
                    .select(comment::post_id)
                    .inner_join(user::table)
                    .into_boxed();
                let subquery = apply_str_filter!(comments, user::name, filter.unnegated());
                Ok(apply_subquery_filter!(query, post::id, filter, subquery))
            }
            Token::NoteText => {
                let post_notes = post_note::table.select(post_note::post_id).into_boxed();
                let subquery = apply_str_filter!(post_notes, post_note::text, filter.unnegated());
                Ok(apply_subquery_filter!(query, post::id, filter, subquery))
            }
            Token::TagCount => apply_filter!(query, post_statistics::tag_count, filter, i64),
            Token::CommentCount => apply_filter!(query, post_statistics::comment_count, filter, i64),
            Token::RelationCount => apply_filter!(query, post_statistics::relation_count, filter, i64),
            Token::NoteCount => apply_filter!(query, post_statistics::note_count, filter, i64),
            Token::FavCount => apply_filter!(query, post_statistics::favorite_count, filter, i64),
            Token::FeatureCount => apply_filter!(query, post_statistics::feature_count, filter, i64),
            Token::CommentTime => {
                let comments = comment::table.select(comment::post_id).into_boxed();
                let subquery = apply_time_filter!(comments, comment::creation_time, filter.unnegated())?;
                Ok(apply_subquery_filter!(query, post::id, filter, subquery))
            }
            Token::FavTime => {
                let post_favorites = post_favorite::table.select(post_favorite::post_id).into_boxed();
                let subquery = apply_time_filter!(post_favorites, post_favorite::time, filter.unnegated())?;
                Ok(apply_subquery_filter!(query, post::id, filter, subquery))
            }
            Token::FeatureTime => {
                let post_features = post_feature::table.select(post_feature::post_id).into_boxed();
                let subquery = apply_time_filter!(post_features, post_feature::time, filter.unnegated())?;
                Ok(apply_subquery_filter!(query, post::id, filter, subquery))
            }
            Token::Special => apply_special_filter(query, *filter, client),
        })
}

pub fn get_ordered_ids(
    conn: &mut PgConnection,
    unsorted_query: BoxedQuery,
    search_criteria: &SearchCriteria<Token>,
) -> QueryResult<Vec<i64>> {
    // If random sort specified, no other sorts matter
    if search_criteria.random_sort {
        define_sql_function!(fn random() -> BigInt);
        return match search_criteria.extra_args {
            Some(args) => unsorted_query.order(random()).offset(args.offset).limit(args.limit),
            None => unsorted_query.order(random()),
        }
        .load(conn);
    }

    // Add default sort if none specified
    let sorts = if search_criteria.has_sort() {
        search_criteria.sorts.as_slice()
    } else {
        &[ParsedSort {
            kind: Token::Id,
            order: Order::default(),
        }]
    };

    let query = sorts.iter().fold(unsorted_query, |query, sort| match sort.kind {
        Token::Id => apply_sort!(query, post::id, sort),
        Token::FileSize => apply_sort!(query, post::file_size, sort),
        Token::Width => apply_sort!(query, post::width, sort),
        Token::Height => apply_sort!(query, post::height, sort),
        Token::Area => apply_sort!(query, post::width * post::height, sort),
        Token::AspectRatio => apply_sort!(query, aspect_ratio(), sort),
        Token::Safety => apply_sort!(query, post::safety, sort),
        Token::Type => apply_sort!(query, post::type_, sort),
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
        Token::CommentTime => apply_sort!(query, post_statistics::last_comment_time, sort),
        Token::FavTime => apply_sort!(query, post_statistics::last_favorite_time, sort),
        Token::FeatureTime => apply_sort!(query, post_statistics::last_feature_time, sort),
        Token::ContentChecksum | Token::NoteText | Token::Special => panic!("Invalid sort-style token!"),
    });
    match search_criteria.extra_args {
        Some(args) => query.offset(args.offset).limit(args.limit),
        None => query,
    }
    .load(conn)
}

#[derive(Clone, Copy, EnumString)]
#[strum(serialize_all = "kebab-case")]
enum SpecialToken {
    Liked,
    Disliked,
    Fav,
    Tumbleweed,
}

type Bind = UncheckedBind<SqlLiteral<Float, UncheckedBind<SqlLiteral<Float>, post::width>>, post::height>;
fn aspect_ratio() -> SqlLiteral<Float, Bind> {
    sql("CAST(")
        .bind(post::width)
        .sql(" AS REAL) / CAST(")
        .bind(post::height)
        .sql(" AS REAL)")
}

fn apply_special_filter<'a>(
    query: BoxedQuery<'a>,
    filter: UnparsedFilter<'a, Token>,
    client: Client,
) -> Result<BoxedQuery<'a>, Error> {
    let special_token = SpecialToken::from_str(filter.criteria).map_err(Box::from)?;
    match special_token {
        SpecialToken::Liked => client.id.ok_or(Error::NotLoggedIn).map(|client_id| {
            let subquery = post_score::table
                .select(post_score::post_id)
                .filter(post_score::user_id.eq(client_id))
                .filter(post_score::score.eq(1));
            apply_subquery_filter!(query, post::id, filter, subquery)
        }),
        SpecialToken::Disliked => client.id.ok_or(Error::NotLoggedIn).map(|client_id| {
            let subquery = post_score::table
                .select(post_score::post_id)
                .filter(post_score::user_id.eq(client_id))
                .filter(post_score::score.eq(-1));
            apply_subquery_filter!(query, post::id, filter, subquery)
        }),
        SpecialToken::Fav => client.id.ok_or(Error::NotLoggedIn).map(|client_id| {
            let subquery = post_favorite::table
                .select(post_favorite::post_id)
                .filter(post_favorite::user_id.eq(client_id));
            apply_subquery_filter!(query, post::id, filter, subquery)
        }),
        SpecialToken::Tumbleweed => {
            // A score of 0 doesn't necessarily mean no ratings, so we count them with a subquery
            let subquery = post_statistics::table
                .select(post_statistics::post_id)
                .left_join(post_score::table.on(post_score::post_id.eq(post_statistics::post_id)))
                .filter(post_statistics::favorite_count.eq(0))
                .filter(post_statistics::comment_count.eq(0))
                .group_by(post_statistics::post_id)
                .having(count(post_score::post_id).eq(0))
                .into_boxed();
            Ok(apply_subquery_filter!(query, post::id, filter, subquery))
        }
    }
}
