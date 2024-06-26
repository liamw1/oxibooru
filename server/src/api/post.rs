use crate::api;
use crate::api::comment::CommentInfo;
use crate::api::micro::{MicroPool, MicroPost, MicroTag, MicroUser};
use crate::auth::content;
use crate::model::comment::Comment;
use crate::model::enums::MimeType;
use crate::model::enums::{PostSafety, PostType};
use crate::model::post::{Post, PostFavorite, PostNote, PostTag};
use crate::model::tag::Tag;
use crate::model::user::User;
use crate::schema::{post, post_favorite, post_score, post_tag, tag, user};
use crate::util::DateTime;
use diesel::prelude::*;
use serde::Serialize;
use std::convert::Infallible;
use warp::hyper::body::Bytes;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list_posts = warp::get()
        .and(warp::path!("posts"))
        .and(api::auth())
        .and(warp::body::bytes())
        .and_then(list_posts_endpoint);

    list_posts
}

#[derive(Serialize)]
struct PostNoteInfo {
    polygon: Vec<u8>, // Probably not correct type, TODO
    text: String,
}

impl PostNoteInfo {
    fn new(note: PostNote) -> Self {
        PostNoteInfo {
            polygon: note.polygon,
            text: note.text,
        }
    }
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
    tags: Vec<MicroTag>,
    relations: Vec<MicroPost>,
    notes: Vec<PostNoteInfo>,
    user: Option<MicroUser>,
    score: i64,
    own_score: Option<i32>,
    own_favorite: bool,
    tag_count: i64,
    favorite_count: i64,
    comment_count: i64,
    note_count: i64,
    feature_count: i64,
    relation_count: i64,
    last_feature_time: Option<DateTime>,
    favorited_by: Vec<MicroUser>,
    has_custom_thumbnail: bool,
    mime_type: MimeType,
    comments: Vec<CommentInfo>,
    pools: Vec<MicroPool>,
}
type PagedPostInfo = api::PagedResponse<PostInfo>;

impl PostInfo {
    fn new(conn: &mut PgConnection, post: Post, client: Option<i32>) -> Result<PostInfo, api::Error> {
        let content_url = content::post_content_url(&post);
        let thumbnail_url = content::post_thumbnail_url(&post);
        let tags = PostTag::belonging_to(&post)
            .inner_join(tag::table.on(post_tag::tag_id.eq(tag::id)))
            .select(Tag::as_select())
            .load(conn)?;
        let micro_tags = tags
            .into_iter()
            .map(|tag| MicroTag::new(conn, tag))
            .collect::<Result<_, _>>()?;
        let related_posts = post.related_posts(conn)?;
        let micro_relations = related_posts.iter().map(|post| MicroPost::new(&post)).collect();
        let notes = PostNote::belonging_to(&post).select(PostNote::as_select()).load(conn)?;
        let score = post.score(conn)?;
        let owner = match post.user_id {
            Some(id) => Some(user::table.find(id).select(User::as_select()).first(conn)?),
            None => None,
        };
        let client_score = match client {
            Some(client_id) => post_score::table
                .find((post.id, client_id))
                .select(post_score::score)
                .load(conn)?,
            None => Vec::new(),
        };
        let client_favorited = match client {
            Some(client_id) => post_favorite::table
                .find((post.id, client_id))
                .count()
                .first(conn)
                .map(|n: i64| n > 0)?,
            None => false,
        };
        let tag_count = post.tag_count(conn)?;
        let favorite_count = post.favorite_count(conn)?;
        let note_count = notes.len() as i64;
        let feature_count = post.feature_count(conn)?;
        let comments = Comment::belonging_to(&post).select(Comment::as_select()).load(conn)?;
        let comment_info = match client {
            Some(client_id) => comments
                .into_iter()
                .map(|comment| CommentInfo::new(conn, comment, client_id))
                .collect::<Result<_, _>>()?,
            None => Vec::new(),
        };
        let favorited_by = PostFavorite::belonging_to(&post)
            .inner_join(user::table.on(post_favorite::user_id.eq(user::id)))
            .select(User::as_select())
            .load(conn)?;
        let pools_in = post.pools_in(conn)?;

        Ok(PostInfo {
            version: post.last_edit_time.clone().into(),
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
            content_url,
            thumbnail_url,
            flags: post.flags,
            tags: micro_tags,
            relations: micro_relations,
            notes: notes.into_iter().map(|note| PostNoteInfo::new(note)).collect(),
            user: owner.map(MicroUser::new),
            score,
            own_score: client_score.first().map(|&s| s),
            own_favorite: client_favorited,
            tag_count,
            favorite_count,
            comment_count: comment_info.len() as i64,
            note_count,
            feature_count,
            relation_count: related_posts.len() as i64,
            last_feature_time: None, // TODO
            favorited_by: favorited_by.into_iter().map(MicroUser::new).collect(),
            has_custom_thumbnail: false, // TODO
            mime_type: post.mime_type,
            comments: comment_info,
            pools: pools_in
                .into_iter()
                .map(|pool| MicroPool::new(conn, pool))
                .collect::<Result<_, _>>()?,
        })
    }
}

async fn list_posts_endpoint(auth_result: api::AuthenticationResult, body: Bytes) -> Result<api::Reply, Infallible> {
    Ok(auth_result
        .and_then(|client| api::parse_body(&body).and_then(|parsed_body| list_posts(parsed_body, client.as_ref())))
        .into())
}

fn list_posts(body: api::PagedRequest, client: Option<&User>) -> Result<PagedPostInfo, api::Error> {
    api::verify_privilege(api::client_access_level(client), "posts:list")?;

    let client_id = client.map(|user| user.id);
    let offset = body.offset.unwrap_or(0);
    let limit = body.limit.unwrap_or(40);

    let mut conn = crate::establish_connection()?;
    let posts = post::table
        .select(Post::as_select())
        .limit(limit)
        .offset(offset)
        .load(&mut conn)?;

    Ok(PagedPostInfo {
        query: body.query.unwrap_or(String::new()),
        offset,
        limit,
        total: Post::count(&mut conn)?,
        results: posts
            .into_iter()
            .map(|post| PostInfo::new(&mut conn, post, client_id))
            .collect::<Result<_, _>>()?,
    })
}
