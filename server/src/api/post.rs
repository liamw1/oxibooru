use crate::api;
use crate::model::enums::{MimeType, UserRank};
use crate::model::enums::{PostSafety, PostType};
use crate::model::post::Post;
use crate::util::DateTime;
use diesel::prelude::*;
use serde::Serialize;
use warp::Rejection;

pub async fn list_posts(auth_result: api::AuthenticationResult) -> Result<api::Reply, Rejection> {
    Ok(api::access_level(auth_result).and_then(read_posts).into())
}

#[derive(Serialize)]
struct PagedPostInfo {
    query: String,
    offset: i32,
    limit: i32,
    total: i32,
    results: Vec<PostInfo>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PostInfo {
    version: DateTime,
    id: i32,
    creation_time: DateTime,
    last_edit_time: DateTime,
    safety: PostSafety,
    source: Option<String>,
    type_: PostType,
    checksum: String,
    #[serde(rename = "checksumMD5")]
    checksum_md5: Option<String>,
    canvas_width: i32,
    canvas_height: i32,
    content_url: String,
    thumbnail_url: String,
    flags: Option<String>,
    tags: String,
    relations: String,
    notes: String,
    user: String,
    score: String,
    own_score: String,
    own_favorite: String,
    tag_count: i64,
    favorite_count: i64,
    comment_count: i64,
    note_count: i64,
    feature_count: i64,
    relation_count: i64,
    last_feature_time: DateTime,
    favorited_by: String,
    has_custom_thumbnail: bool,
    mime_type: MimeType,
    comments: String,
    pools: String,
}

/*
impl PostInfo {
    fn new(conn: &mut PgConnection, post: Post) -> Result<PostInfo, api::Error> {
        Ok(PostInfo {
            version: post.last_edit_time,
            id: post.id,
            creation_time: post.creation_time,
            last_edit_time: post.last_edit_time,
            safety: post.safety,
            source: post.source,
            type_: post.type_,
            checksum: post.checksum,
            checksum_md5: post.checksum_md5,
            canvas_width: post.width,
            canvas_height: post.height,
        })
    }
}
*/

fn read_posts(access_level: UserRank) -> Result<PagedPostInfo, api::Error> {
    api::validate_privilege(access_level, "posts:list")?;

    Err(api::Error::ResourceDoesNotExist)
}
