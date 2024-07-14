use crate::model::enums::{PostSafety, PostType};
use crate::model::post::Post;
use crate::schema::{
    comment, pool_post, post, post_favorite, post_feature, post_note, post_relation, post_score, post_tag, tag_name,
    user,
};
use crate::search::Error;
use crate::search::{ParsedSort, UnparsedFilter};
use crate::{apply_filter, apply_having_clause, apply_sort, apply_str_filter, apply_time_filter};
use diesel::dsl::{self, AsSelect, IntoBoxed, Select};
use diesel::pg::Pg;
use diesel::prelude::*;
use std::str::FromStr;
use strum::EnumString;

pub type BoxedQuery<'a> = IntoBoxed<'a, Select<post::table, AsSelect<Post, Pg>>, Pg>;

pub fn build_query(client: Option<i32>, client_query: &str) -> Result<BoxedQuery, Error> {
    let mut filters: Vec<UnparsedFilter<Token>> = Vec::new();
    let mut sorts: Vec<ParsedSort<Token>> = Vec::new();
    let mut special_tokens: Vec<SpecialToken> = Vec::new();
    let mut random_sort = false;

    for mut term in client_query.split_whitespace() {
        let negated = term.chars().nth(0) == Some('-');
        if negated {
            term = term.strip_prefix('-').unwrap();
        }

        match term.split_once(':') {
            Some(("special", value)) => special_tokens.push(SpecialToken::from_str(value).map_err(Box::from)?),
            Some(("sort", "random")) => random_sort = true,
            Some(("sort", value)) => {
                let kind = Token::from_str(value).map_err(Box::from)?;
                sorts.push(ParsedSort { kind, negated });
            }
            Some((key, criteria)) => {
                filters.push(UnparsedFilter {
                    kind: Token::from_str(key).map_err(Box::from)?,
                    criteria,
                    negated,
                });
            }
            None => filters.push(UnparsedFilter {
                kind: Token::Tag,
                criteria: term,
                negated,
            }),
        }
    }

    let query = filters
        .into_iter()
        .try_fold(post::table.select(Post::as_select()).into_boxed(), |query, filter| match filter.kind {
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
                Ok(query
                    .filter(post::user_id.is_not_null())
                    .filter(post::user_id.assume_not_null().eq_any(subquery)))
            }
            Token::Pool => {
                let pool_posts = pool_post::table.select(pool_post::post_id).into_boxed();
                apply_filter!(pool_posts, pool_post::pool_id, filter, i32)
                    .map(|subquery| query.filter(post::id.eq_any(subquery)))
            }
            Token::TagCount => {
                let post_tags = post::table
                    .select(post::id)
                    .left_join(post_tag::table)
                    .group_by(post::id)
                    .into_boxed();
                apply_having_clause!(post_tags, post_tag::post_id, filter)
                    .map(|subquery| query.filter(post::id.eq_any(subquery)))
            }
            Token::CommentCount => {
                let comments = post::table
                    .select(post::id)
                    .left_join(comment::table)
                    .group_by(post::id)
                    .into_boxed();
                apply_having_clause!(comments, comment::post_id, filter)
                    .map(|subquery| query.filter(post::id.eq_any(subquery)))
            }
            Token::FavCount => {
                let post_favorites = post::table
                    .select(post::id)
                    .left_join(post_favorite::table)
                    .group_by(post::id)
                    .into_boxed();
                apply_having_clause!(post_favorites, post_favorite::post_id, filter)
                    .map(|subquery| query.filter(post::id.eq_any(subquery)))
            }
            Token::NoteCount => {
                let post_notes = post::table
                    .select(post::id)
                    .left_join(post_note::table)
                    .group_by(post::id)
                    .into_boxed();
                apply_having_clause!(post_notes, post_note::post_id, filter)
                    .map(|subquery| query.filter(post::id.eq_any(subquery)))
            }
            Token::NoteText => {
                let post_notes = post_note::table.select(post_note::post_id).into_boxed();
                let subquery = apply_str_filter!(post_notes, post_note::text, filter);
                Ok(query.filter(post::id.eq_any(subquery)))
            }
            Token::RelationCount => {
                let post_relations = post::table
                    .select(post::id)
                    .left_join(post_relation::table)
                    .group_by(post::id)
                    .into_boxed();
                apply_having_clause!(post_relations, post_relation::parent_id, filter)
                    .map(|subquery| query.filter(post::id.eq_any(subquery)))
            }
            Token::FeatureCount => {
                let post_features = post::table
                    .select(post::id)
                    .left_join(post_feature::table)
                    .group_by(post::id)
                    .into_boxed();
                apply_having_clause!(post_features, post_feature::post_id, filter)
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

    let query = special_tokens.into_iter().try_fold(query, |query, token| match token {
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
                .having(dsl::count(post_score::post_id).eq(0))
                .into_boxed();
            let favorite_subquery = post::table
                .select(post::id)
                .left_join(post_favorite::table)
                .group_by(post::id)
                .having(dsl::count(post_favorite::post_id).eq(0))
                .into_boxed();
            let comment_subquery = post::table
                .select(post::id)
                .left_join(comment::table)
                .group_by(post::id)
                .having(dsl::count(comment::post_id).eq(0))
                .into_boxed();

            Ok(query
                .filter(post::id.eq_any(score_subquery))
                .filter(post::id.eq_any(favorite_subquery))
                .filter(post::id.eq_any(comment_subquery)))
        }
    })?;

    if random_sort {
        define_sql_function!(fn random() -> Integer);
        return Ok(query.order_by(random()));
    }

    Ok(sorts.into_iter().fold(query, |query, sort| match sort.kind {
        Token::Id => apply_sort!(query, post::id, sort),
        Token::FileSize => apply_sort!(query, post::file_size, sort),
        Token::ImageWidth => apply_sort!(query, post::width, sort),
        Token::ImageHeight => apply_sort!(query, post::height, sort),
        Token::ImageArea => apply_sort!(query, post::width * post::height, sort),
        Token::ImageAspectRatio => apply_sort!(query, post::width / post::height, sort),
        Token::Safety => apply_sort!(query, post::safety, sort),
        Token::Type => apply_sort!(query, post::type_, sort),
        Token::ContentChecksum => apply_sort!(query, post::checksum, sort),
        Token::CreationTime => apply_sort!(query, post::creation_time, sort),
        Token::LastEditTime => apply_sort!(query, post::last_edit_time, sort),

        Token::Tag => unimplemented!(),
        Token::Uploader => unimplemented!(),
        Token::Pool => unimplemented!(),
        Token::TagCount => unimplemented!(),
        Token::CommentCount => unimplemented!(),
        Token::FavCount => unimplemented!(),
        Token::NoteCount => unimplemented!(),
        Token::NoteText => unimplemented!(),
        Token::RelationCount => unimplemented!(),
        Token::FeatureCount => unimplemented!(),
        Token::CommentTime => unimplemented!(),
        Token::FavTime => unimplemented!(),
        Token::FeatureTime => unimplemented!(),
    }))
}

#[derive(Clone, Copy, EnumString)]
#[strum(serialize_all = "kebab-case")]
#[strum(use_phf)]
enum Token {
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
    TagCount,
    CommentCount,
    FavCount,
    NoteCount,
    NoteText,
    RelationCount,
    FeatureCount,
    #[strum(serialize = "comment-date", serialize = "comment-time")]
    CommentTime,
    #[strum(serialize = "fav-date", serialize = "fav-time")]
    FavTime,
    #[strum(serialize = "feature-date", serialize = "feature-time")]
    FeatureTime,
}

#[derive(Clone, Copy, EnumString)]
#[strum(serialize_all = "kebab-case")]
enum SpecialToken {
    Liked,
    Disliked,
    Fav,
    Tumbleweed,
}
