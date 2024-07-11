use crate::schema::post;
use crate::search::filter::*;
use crate::search::{Error, ParsedSort, UnparsedFilter};
use diesel::prelude::*;
use std::str::FromStr;
use strum::EnumString;

pub fn post_query(client_query: &str) -> Result<(), Error> {
    let mut post_column_filters: Vec<UnparsedFilter<PostColumnToken>> = Vec::new();
    let mut post_column_sorts: Vec<ParsedSort<PostColumnToken>> = Vec::new();
    let mut join_filters: Vec<UnparsedFilter<JoinToken>> = Vec::new();
    let mut join_sorts: Vec<ParsedSort<JoinToken>> = Vec::new();
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
                if let Ok(kind) = PostColumnToken::from_str(value) {
                    post_column_sorts.push(ParsedSort { kind, negated });
                } else {
                    let kind = JoinToken::from_str(value).map_err(Box::from)?;
                    join_sorts.push(ParsedSort { kind, negated });
                }
            }
            Some((key, criteria)) => {
                if let Ok(kind) = PostColumnToken::from_str(key) {
                    post_column_filters.push(UnparsedFilter {
                        kind,
                        criteria,
                        negated,
                    });
                } else {
                    let kind = JoinToken::from_str(key).map_err(Box::from)?;
                    join_filters.push(UnparsedFilter {
                        kind,
                        criteria,
                        negated,
                    });
                }
            }
            None => join_filters.push(UnparsedFilter {
                kind: JoinToken::Tag,
                criteria: term,
                negated,
            }),
        }
    }

    let query = post_column_filters
        .into_iter()
        .try_fold(post::table.into_boxed(), |query, filter| match filter.kind {
            PostColumnToken::Id => apply_i32_filter(query, post::id, filter),
            PostColumnToken::FileSize => apply_i64_filter(query, post::file_size, filter),
            PostColumnToken::ImageWidth => apply_i32_filter(query, post::width, filter),
            PostColumnToken::ImageHeight => apply_i32_filter(query, post::height, filter),
            PostColumnToken::ImageArea => unimplemented!(),
            PostColumnToken::ImageAspectRatio => unimplemented!(),
            PostColumnToken::Safety => apply_i16_filter(query, post::safety, filter),
            PostColumnToken::Type => apply_i16_filter(query, post::type_, filter),
            PostColumnToken::ContentChecksum => Ok(apply_str_filter(query, post::checksum, filter)),
            PostColumnToken::CreationTime => apply_time_filter(query, post::creation_time, filter),
            PostColumnToken::LastEditTime => apply_time_filter(query, post::last_edit_time, filter),
        });

    Ok(())
}

#[derive(Clone, Copy, EnumString)]
#[strum(serialize_all = "kebab-case")]
#[strum(use_phf)]
enum PostColumnToken {
    Id,
    FileSize,
    #[strum(serialize = "width", serialize = "image-width")]
    ImageWidth,
    #[strum(serialize = "height", serialize = "image-height")]
    ImageHeight,
    #[strum(serialize = "area", serialize = "image-area")]
    ImageArea,
    #[strum(serialize = "ar", serialize = "image-ar", serialize = "image-aspect-ratio")]
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
}

#[derive(Clone, Copy, EnumString)]
#[strum(serialize_all = "kebab-case")]
#[strum(use_phf)]
enum JoinToken {
    Tag,
    #[strum(serialize = "submit", serialize = "upload", serialize = "uploader")]
    Uploader,
    Comment,
    Fav,
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
