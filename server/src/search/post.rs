use crate::model::enums::{PostSafety, PostType};
use crate::model::post::Post;
use crate::schema::{
    comment, pool_post, post, post_favorite, post_feature, post_note, post_relation, post_score, post_tag, tag_name,
    user,
};
use crate::search::Error;
use crate::search::{ParsedSort, UnparsedFilter};
use crate::{apply_filter, apply_having_clause, apply_sort, apply_str_filter, apply_time_filter};
use diesel::dsl::{self, AsSelect, Eq, GroupBy, InnerJoinOn, IntoBoxed, LeftJoin, Select};
use diesel::pg::Pg;
use diesel::prelude::*;
use std::str::FromStr;
use strum::EnumString;

pub type BoxedQuery<'a> = IntoBoxed<
    'a,
    LeftJoin<
        LeftJoin<
            LeftJoin<
                LeftJoin<
                    LeftJoin<
                        LeftJoin<
                            LeftJoin<
                                LeftJoin<
                                    LeftJoin<GroupBy<Select<post::table, AsSelect<Post, Pg>>, post::id>, user::table>,
                                    comment::table,
                                >,
                                pool_post::table,
                            >,
                            post_score::table,
                        >,
                        post_favorite::table,
                    >,
                    post_feature::table,
                >,
                post_relation::table,
            >,
            post_note::table,
        >,
        InnerJoinOn<post_tag::table, tag_name::table, Eq<post_tag::tag_id, tag_name::tag_id>>,
    >,
    Pg,
>;

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

    /*
        Performing all potentially necessary joins here because diesel makes dynamically joining very difficult.
        Preemptively joining all these tables can make for some pretty inefficient queries...
        If Postgres had DISTINCT left join elimination the query planner could remove the unneeded ones automatically.
        TODO: Get rid of the unnecessary joins somehow.
    */
    let joined_tables = post::table
        .select(Post::as_select())
        .group_by(post::id)
        .left_join(user::table)
        .left_join(comment::table)
        .left_join(pool_post::table)
        .left_join(post_score::table)
        .left_join(post_favorite::table)
        .left_join(post_feature::table)
        .left_join(post_relation::table)
        .left_join(post_note::table)
        .left_join(post_tag::table.inner_join(tag_name::table.on(post_tag::tag_id.eq(tag_name::tag_id))));

    let query = filters
        .into_iter()
        .try_fold(joined_tables.into_boxed(), |query, filter| match filter.kind {
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

            Token::Tag => Ok(apply_str_filter!(query, tag_name::name, filter)),
            Token::Uploader => Ok(apply_str_filter!(query, user::name, filter)),
            Token::Pool => apply_filter!(query, pool_post::pool_id, filter, i32),
            Token::TagCount => apply_having_clause!(query, post_tag::tag_id, filter),
            Token::CommentCount => apply_having_clause!(query, comment::id, filter),
            Token::FavCount => apply_having_clause!(query, post_favorite::user_id, filter),
            Token::NoteCount => apply_having_clause!(query, post_note::id, filter),
            Token::NoteText => Ok(apply_str_filter!(query, post_note::text, filter)),
            Token::RelationCount => apply_having_clause!(query, post_relation::child_id, filter),
            Token::FeatureCount => apply_having_clause!(query, post_feature::id, filter),
            Token::CommentTime => apply_time_filter!(query, comment::creation_time, filter),
            Token::FavTime => apply_time_filter!(query, post_favorite::time, filter),
            Token::FeatureTime => apply_time_filter!(query, post_feature::time, filter),
        })?;

    let query = special_tokens.into_iter().try_fold(query, |query, token| match token {
        SpecialToken::Liked => client.ok_or(Error::NotLoggedIn).map(|client_id| {
            query
                .filter(post_score::user_id.eq(client_id))
                .filter(post_score::score.eq(1))
        }),
        SpecialToken::Disliked => client.ok_or(Error::NotLoggedIn).map(|client_id| {
            query
                .filter(post_score::user_id.eq(client_id))
                .filter(post_score::score.eq(-1))
        }),
        SpecialToken::Fav => client
            .ok_or(Error::NotLoggedIn)
            .map(|client_id| query.filter(post_favorite::user_id.eq(client_id))),
        SpecialToken::Tumbleweed => Ok(query.having(
            dsl::count(post_score::user_id)
                .eq(0)
                .and(dsl::count(post_favorite::user_id).eq(0))
                .and(dsl::count(comment::user_id).eq(0)),
        )),
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

        Token::Tag => apply_sort!(query, tag_name::name, sort),
        Token::Uploader => apply_sort!(query, user::name, sort),
        Token::Pool => apply_sort!(query, pool_post::pool_id, sort),
        Token::TagCount => apply_sort!(query, dsl::count(post_tag::tag_id), sort),
        Token::CommentCount => apply_sort!(query, dsl::count(comment::id), sort),
        Token::FavCount => apply_sort!(query, dsl::count(post_favorite::user_id), sort),
        Token::NoteCount => apply_sort!(query, dsl::count(post_note::id), sort),
        Token::NoteText => apply_sort!(query, post_note::text, sort),
        Token::RelationCount => apply_sort!(query, dsl::count(post_relation::child_id), sort),
        Token::FeatureCount => apply_sort!(query, dsl::count(post_feature::id), sort),
        Token::CommentTime => apply_sort!(query, comment::creation_time, sort),
        Token::FavTime => apply_sort!(query, post_favorite::time, sort),
        Token::FeatureTime => apply_sort!(query, post_feature::time, sort),
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
