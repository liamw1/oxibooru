use crate::api::comment::CommentInfo;
use crate::api::micro::{MicroPool, MicroPost, MicroTag, MicroUser};
use crate::auth::content;
use crate::image::signature;
use crate::model::comment::Comment;
use crate::model::enums::MimeType;
use crate::model::enums::{PostSafety, PostType};
use crate::model::post::{NewPost, NewPostSignature, Post, PostFavorite, PostNote, PostSignature, PostTag};
use crate::model::tag::Tag;
use crate::model::user::User;
use crate::schema::{post, post_favorite, post_score, post_signature, post_tag, tag, user};
use crate::util::DateTime;
use crate::{api, config};
use diesel::prelude::*;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::path::PathBuf;
use warp::hyper::body::Bytes;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list_posts = warp::get()
        .and(warp::path!("posts"))
        .and(api::auth())
        .and(warp::query())
        .and_then(list_posts_endpoint);
    let get_post = warp::get()
        .and(warp::path!("post" / i32))
        .and(api::auth())
        .and_then(get_post_endpoint);
    let get_post_around = warp::get()
        .and(warp::path!("post" / i32 / "around"))
        .and(api::auth())
        .and_then(get_post_around_endpoint);
    let reverse_search = warp::post()
        .and(warp::path!("posts" / "reverse-search"))
        .and(api::auth())
        .and(warp::body::bytes())
        .and_then(reverse_search_from_temporary_endpoint);
    let post_post = warp::post()
        .and(warp::path!("posts"))
        .and(api::auth())
        .and(warp::body::bytes())
        .and_then(post_post_endpoint);

    list_posts
        .or(get_post)
        .or(get_post_around)
        .or(reverse_search)
        .or(post_post)
}

static THUMBNAIL_WIDTH: Lazy<u32> = Lazy::new(|| {
    config::read_required_table("thumbnails")
        .get("post_width")
        .unwrap_or_else(|| panic!("Config post_width missing from [thumbnails]"))
        .as_integer()
        .unwrap_or_else(|| panic!("Config post_width is not an integer"))
        .try_into()
        .unwrap_or_else(|value| panic!("Config post_width ({value}) cannot be represented as u32"))
});
static THUMBNAIL_HEIGHT: Lazy<u32> = Lazy::new(|| {
    config::read_required_table("thumbnails")
        .get("post_height")
        .unwrap_or_else(|| panic!("Config post_height missing from [thumbnails]"))
        .as_integer()
        .unwrap_or_else(|| panic!("Config post_height is not an integer"))
        .try_into()
        .unwrap_or_else(|value| panic!("Config post_height ({value}) cannot be represented as u32"))
});

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
    fn new(conn: &mut PgConnection, post: Post, client: Option<i32>) -> QueryResult<PostInfo> {
        let content_url = content::post_content_url(&post);
        let thumbnail_url = content::post_thumbnail_url(&post);
        let tags = PostTag::belonging_to(&post)
            .inner_join(tag::table.on(post_tag::tag_id.eq(tag::id)))
            .select(Tag::as_select())
            .load(conn)?;
        let micro_tags = tags
            .into_iter()
            .map(|tag| MicroTag::new(conn, tag))
            .collect::<QueryResult<_>>()?;
        let related_posts = post.related_posts(conn)?;
        let micro_relations = related_posts.iter().map(|post| MicroPost::new(&post)).collect();
        let notes = PostNote::belonging_to(&post).select(PostNote::as_select()).load(conn)?;
        let score = post.score(conn)?;
        let owner = post
            .user_id
            .map(|id| user::table.find(id).select(User::as_select()).first(conn))
            .transpose()?;
        let client_score = client
            .map(|id| {
                post_score::table
                    .find((post.id, id))
                    .select(post_score::score)
                    .first::<i32>(conn)
                    .optional()
            })
            .transpose()?;
        let client_favorited = client
            .map(|id| {
                post_favorite::table
                    .find((post.id, id))
                    .count()
                    .first(conn)
                    .map(|n: i64| n > 0)
            })
            .transpose()?;
        let tag_count = post.tag_count(conn)?;
        let favorite_count = post.favorite_count(conn)?;
        let note_count = notes.len() as i64;
        let feature_count = post.feature_count(conn)?;
        let comments = Comment::belonging_to(&post).select(Comment::as_select()).load(conn)?;
        let comment_info = comments
            .into_iter()
            .map(|comment| CommentInfo::new(conn, comment, client))
            .collect::<QueryResult<Vec<_>>>()?;
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
            own_score: client_score.flatten(),
            own_favorite: client_favorited.unwrap_or(false),
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
                .collect::<QueryResult<_>>()?,
        })
    }
}

async fn list_posts_endpoint(
    auth_result: api::AuthenticationResult,
    query: api::PagedQuery,
) -> Result<api::Reply, Infallible> {
    Ok(auth_result.and_then(|client| list_posts(query, client.as_ref())).into())
}

fn list_posts(query_info: api::PagedQuery, client: Option<&User>) -> Result<PagedPostInfo, api::Error> {
    api::verify_privilege(api::client_access_level(client), "posts:list")?;

    let client_id = client.map(|user| user.id);
    let offset = query_info.offset.unwrap_or(0);
    let limit = query_info.limit.unwrap_or(40);

    let mut conn = crate::establish_connection()?;
    let posts = post::table
        .select(Post::as_select())
        .limit(limit)
        .offset(offset)
        .load(&mut conn)?;

    Ok(PagedPostInfo {
        query: query_info.query.unwrap_or(String::new()),
        offset,
        limit,
        total: Post::count(&mut conn)?,
        results: posts
            .into_iter()
            .map(|post| PostInfo::new(&mut conn, post, client_id))
            .collect::<QueryResult<_>>()?,
    })
}

async fn get_post_endpoint(post_id: i32, auth_result: api::AuthenticationResult) -> Result<api::Reply, Infallible> {
    Ok(auth_result.and_then(|client| get_post(post_id, client.as_ref())).into())
}

fn get_post(post_id: i32, client: Option<&User>) -> Result<PostInfo, api::Error> {
    let client_id = client.map(|user| user.id);

    let mut conn = crate::establish_connection()?;
    let post = post::table.find(post_id).select(Post::as_select()).first(&mut conn)?;
    PostInfo::new(&mut conn, post, client_id).map_err(api::Error::from)
}

#[derive(Serialize)]
struct PostNeighbors {
    prev: Option<PostInfo>,
    next: Option<PostInfo>,
}

async fn get_post_around_endpoint(
    post_id: i32,
    auth_result: api::AuthenticationResult,
) -> Result<api::Reply, Infallible> {
    Ok(auth_result
        .and_then(|client| get_post_around(post_id, client.as_ref()))
        .into())
}

fn get_post_around(post_id: i32, client: Option<&User>) -> Result<PostNeighbors, api::Error> {
    let client_id = client.map(|user| user.id);
    let mut conn = crate::establish_connection()?;

    let previous_post = post::table
        .select(Post::as_select())
        .filter(post::id.lt(post_id))
        .first(&mut conn)
        .optional()?;
    let prev = previous_post
        .map(|post| PostInfo::new(&mut conn, post, client_id))
        .transpose()?;

    let next_post = post::table
        .select(Post::as_select())
        .filter(post::id.gt(post_id))
        .first(&mut conn)
        .optional()?;
    let next = next_post
        .map(|post| PostInfo::new(&mut conn, post, client_id))
        .transpose()?;

    Ok(PostNeighbors { prev, next })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContentToken {
    content_token: String,
}

#[derive(Serialize)]
struct SimilarPostInfo {
    distance: f64,
    post: PostInfo,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReverseSearchInfo {
    exact_post: Option<PostInfo>,
    similar_posts: Vec<SimilarPostInfo>,
}

async fn reverse_search_from_temporary_endpoint(
    auth_result: api::AuthenticationResult,
    body: Bytes,
) -> Result<api::Reply, Infallible> {
    Ok(auth_result
        .and_then(|client| {
            api::parse_json_body(&body).and_then(|token| reverse_search_from_temporary(token, client.as_ref()))
        })
        .into())
}

fn reverse_search_from_temporary(token: ContentToken, client: Option<&User>) -> Result<ReverseSearchInfo, api::Error> {
    let (_uuid, extension) = token.content_token.split_once('.').unwrap();
    let content_type = MimeType::from_extension(extension)?;
    let post_type = PostType::from(content_type);
    if post_type != PostType::Image {
        panic!("Unsupported post type!") // TODO
    }

    let data_directory = config::read_required_string("data_dir");
    let path = PathBuf::from(format!("{data_directory}/temporary-uploads/{}", token.content_token));
    let image = image::open(path)?;
    let image_signature = signature::compute_signature(&image);
    let indexes = signature::generate_indexes(&image_signature)
        .into_iter()
        .map(|index| Some(index))
        .collect::<Vec<_>>();

    let mut conn = crate::establish_connection()?;
    let similar_signatures = PostSignature::find_similar(&mut conn, indexes)?; // Might be better served as a method on Post

    let mut similar_posts = Vec::new();
    for post_signature in similar_signatures.into_iter() {
        let distance = signature::normalized_distance(&post_signature.signature, &image_signature);
        if distance < 0.6 {
            let post = post::table
                .find(post_signature.post_id)
                .select(Post::as_select())
                .first(&mut conn)?;
            similar_posts.push(SimilarPostInfo {
                distance,
                post: PostInfo::new(&mut conn, post, client.map(|user| user.id))?,
            });
        }
    }

    Ok(ReverseSearchInfo {
        exact_post: None, // TODO
        similar_posts,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewPostInfo {
    tags: Option<Vec<String>>,
    safety: PostSafety,
    source: Option<String>,
    relations: Option<Vec<i64>>,
    notes: Option<Vec<String>>,
    flags: Option<Vec<String>>, // TODO
    anonymous: Option<bool>,
    content_token: String,
}

async fn post_post_endpoint(auth_result: api::AuthenticationResult, body: Bytes) -> Result<api::Reply, Infallible> {
    Ok(auth_result
        .and_then(|client| api::parse_json_body(&body).and_then(|post_info| post_post(post_info, client.as_ref())))
        .into())
}

fn post_post(post_info: NewPostInfo, client: Option<&User>) -> Result<PostInfo, api::Error> {
    let (_uuid, extension) = post_info.content_token.split_once('.').unwrap();
    let content_type = MimeType::from_extension(extension)?;
    let post_type = PostType::from(content_type);
    if post_type != PostType::Image {
        panic!("Unsupported post type!") // TODO
    }

    let data_directory = config::read_required_string("data_dir");
    let temp_path = PathBuf::from(format!("{data_directory}/temporary-uploads/{}", post_info.content_token));
    let image_size = std::fs::metadata(&temp_path)?.len();
    let image = image::open(&temp_path)?;

    let client_id = client.map(|user| user.id);
    let new_post = NewPost {
        user_id: client_id,
        file_size: image_size as i64,
        width: image.width() as i32,
        height: image.height() as i32,
        safety: post_info.safety,
        type_: post_type,
        mime_type: content_type,
        checksum: "", // TODO
    };

    let mut conn = crate::establish_connection()?;
    let post = diesel::insert_into(post::table)
        .values(&new_post)
        .returning(Post::as_returning())
        .get_result(&mut conn)?;

    let image_signature = signature::compute_signature(&image);
    let new_post_signature = NewPostSignature {
        post_id: post.id,
        signature: &image_signature,
        words: &signature::generate_indexes(&image_signature),
    };
    diesel::insert_into(post_signature::table)
        .values(&new_post_signature)
        .returning(PostSignature::as_returning())
        .get_result(&mut conn)?;

    let posts_folder = PathBuf::from(format!("{data_directory}/posts"));
    if !posts_folder.exists() {
        std::fs::create_dir(&posts_folder)?;
    }
    let post_path = PathBuf::from(format!(
        "{data_directory}/posts/{}_{}.{}",
        post.id,
        content::post_security_hash(post.id),
        post.mime_type.extension()
    ));
    std::fs::rename(temp_path, post_path)?;

    let thumbnail_folder = PathBuf::from(format!("{data_directory}/generated-thumbnails"));
    if !thumbnail_folder.exists() {
        std::fs::create_dir(&thumbnail_folder)?;
    }
    let thumbnail_path = PathBuf::from(format!(
        "{data_directory}/generated-thumbnails/{}_{}.jpg",
        post.id,
        content::post_security_hash(post.id)
    ));
    let thumbnail = image.resize_to_fill(*THUMBNAIL_WIDTH, *THUMBNAIL_HEIGHT, image::imageops::FilterType::Nearest);
    thumbnail.save(thumbnail_path)?;

    PostInfo::new(&mut conn, post, client_id).map_err(api::Error::from)
}
