use crate::api::comment::CommentInfo;
use crate::api::micro::{MicroPool, MicroPost, MicroTag, MicroUser};
use crate::api::AuthResult;
use crate::auth::content;
use crate::image::signature;
use crate::model::comment::Comment;
use crate::model::enums::MimeType;
use crate::model::enums::{PostSafety, PostType};
use crate::model::post::{
    NewPost, NewPostRelation, NewPostSignature, Post, PostFavorite, PostNote, PostSignature, PostTag,
};
use crate::model::tag::Tag;
use crate::model::user::User;
use crate::schema::{post, post_favorite, post_relation, post_score, post_signature, post_tag, tag, user};
use crate::util::DateTime;
use crate::{api, config, filesystem};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list_posts = warp::get()
        .and(warp::path!("posts"))
        .and(api::auth())
        .and(warp::query())
        .map(list_posts)
        .map(api::Reply::from);
    let get_post = warp::get()
        .and(warp::path!("post" / i32))
        .and(api::auth())
        .map(get_post)
        .map(api::Reply::from);
    let get_post_neighbors = warp::get()
        .and(warp::path!("post" / i32 / "around"))
        .and(api::auth())
        .map(get_post_neighbors)
        .map(api::Reply::from);
    let reverse_search = warp::post()
        .and(warp::path!("posts" / "reverse-search"))
        .and(api::auth())
        .and(warp::body::json())
        .map(reverse_search)
        .map(api::Reply::from);
    let post_post = warp::post()
        .and(warp::path!("posts"))
        .and(api::auth())
        .and(warp::body::json())
        .map(create_post)
        .map(api::Reply::from);
    let delete_post = warp::delete()
        .and(warp::path!("post" / i32))
        .and(api::auth())
        .and(warp::body::json())
        .map(delete_post)
        .map(api::Reply::from);

    list_posts
        .or(get_post)
        .or(get_post_neighbors)
        .or(reverse_search)
        .or(post_post)
        .or(delete_post)
}

const MAX_POSTS_PER_PAGE: i64 = 50;
const POST_SIMILARITY_THRESHOLD: f64 = 0.4;

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
    file_size: i64,
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
    // Retrieving all information for now, but will need to add support for partial post queries, TODO
    fn new(conn: &mut PgConnection, post: Post, client: Option<i32>) -> QueryResult<PostInfo> {
        let content_url = content::post_content_url(post.id, post.mime_type);
        let thumbnail_url = content::post_thumbnail_url(post.id);
        let tags = PostTag::belonging_to(&post)
            .inner_join(tag::table.on(post_tag::tag_id.eq(tag::id)))
            .select(Tag::as_select())
            .load(conn)?
            .into_iter()
            .map(|tag| MicroTag::new(conn, tag))
            .collect::<QueryResult<_>>()?;
        let relations = post
            .related_posts(conn)?
            .into_iter()
            .map(|post| MicroPost::new(&post))
            .collect::<Vec<_>>();
        let notes = PostNote::belonging_to(&post)
            .select(PostNote::as_select())
            .load(conn)?
            .into_iter()
            .map(|note| PostNoteInfo::new(note))
            .collect::<Vec<_>>();
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
        let relation_count = relations.len() as i64;
        let feature_count = post.feature_count(conn)?;
        let comments = Comment::belonging_to(&post)
            .select(Comment::as_select())
            .load(conn)?
            .into_iter()
            .map(|comment| CommentInfo::new(conn, comment, client))
            .collect::<QueryResult<Vec<_>>>()?;
        let favorited_by = PostFavorite::belonging_to(&post)
            .inner_join(user::table.on(post_favorite::user_id.eq(user::id)))
            .select(User::as_select())
            .load(conn)?
            .into_iter()
            .map(MicroUser::new)
            .collect::<Vec<_>>();
        let pools = post
            .pools_in(conn)?
            .into_iter()
            .map(|pool| MicroPool::new(conn, pool))
            .collect::<QueryResult<_>>()?;

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
            file_size: post.file_size,
            canvas_width: post.width,
            canvas_height: post.height,
            content_url,
            thumbnail_url,
            flags: post.flags,
            tags,
            relations,
            notes,
            user: owner.map(MicroUser::new),
            score,
            own_score: client_score.flatten(),
            own_favorite: client_favorited.unwrap_or(false),
            tag_count,
            favorite_count,
            comment_count: comments.len() as i64,
            note_count,
            feature_count,
            relation_count,
            last_feature_time: None, // TODO
            favorited_by,
            has_custom_thumbnail: false, // TODO
            mime_type: post.mime_type,
            comments,
            pools,
        })
    }
}

fn list_posts(auth_result: AuthResult, query_info: api::PagedQuery) -> Result<PagedPostInfo, api::Error> {
    let client = auth_result?;
    api::verify_privilege(client.as_ref(), config::privileges().post_list)?;

    let client_id = client.map(|user| user.id);
    let offset = query_info.offset.unwrap_or(0);
    let limit = std::cmp::min(query_info.limit, MAX_POSTS_PER_PAGE);

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

fn get_post(post_id: i32, auth_result: AuthResult) -> Result<PostInfo, api::Error> {
    let client = auth_result?;
    api::verify_privilege(client.as_ref(), config::privileges().post_view)?;

    let mut conn = crate::establish_connection()?;
    let post = post::table.find(post_id).select(Post::as_select()).first(&mut conn)?;
    let client_id = client.map(|user| user.id);
    PostInfo::new(&mut conn, post, client_id).map_err(api::Error::from)
}

#[derive(Serialize)]
struct PostNeighbors {
    prev: Option<PostInfo>,
    next: Option<PostInfo>,
}

fn get_post_neighbors(post_id: i32, auth_result: AuthResult) -> Result<PostNeighbors, api::Error> {
    let client = auth_result?;
    api::verify_privilege(client.as_ref(), config::privileges().post_list)?;

    let client_id = client.map(|user| user.id);
    let mut conn = crate::establish_connection()?;

    let previous_post = post::table
        .select(Post::as_select())
        .filter(post::id.lt(post_id))
        .order_by(post::id.desc())
        .first(&mut conn)
        .optional()?;
    let prev = previous_post
        .map(|post| PostInfo::new(&mut conn, post, client_id))
        .transpose()?;

    let next_post = post::table
        .select(Post::as_select())
        .filter(post::id.gt(post_id))
        .order_by(post::id.asc())
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

fn reverse_search(auth_result: AuthResult, token: ContentToken) -> Result<ReverseSearchInfo, api::Error> {
    let client = auth_result?;
    api::verify_privilege(client.as_ref(), config::privileges().post_reverse_search)?;

    let (_uuid, extension) = token.content_token.split_once('.').unwrap();
    let content_type = MimeType::from_extension(extension)?;
    let post_type = PostType::from(content_type);
    if post_type != PostType::Image {
        panic!("Unsupported post type!") // TODO
    }

    let temp_path = filesystem::temporary_upload_filepath(&token.content_token);
    let image = image::open(temp_path)?;
    let image_signature = signature::compute_signature(&image);
    let image_checksum = content::image_checksum(&image);

    let mut conn = crate::establish_connection()?;

    // Check for exact match
    let client_id = client.map(|user| user.id);
    let exact_post = post::table
        .select(Post::as_select())
        .filter(post::checksum.eq(image_checksum))
        .first(&mut conn)
        .optional()?;
    if exact_post.is_some() {
        return Ok(ReverseSearchInfo {
            exact_post: exact_post
                .map(|post| PostInfo::new(&mut conn, post, client_id))
                .transpose()?,
            similar_posts: Vec::new(),
        });
    }

    // Search for similar images
    let mut similar_posts = Vec::new();
    let similar_signatures = PostSignature::find_similar(&mut conn, signature::generate_indexes(&image_signature))?;
    println!("Found {} similar signatures", similar_signatures.len());
    for post_signature in similar_signatures.into_iter() {
        let distance = signature::normalized_distance(&post_signature.signature, &image_signature);
        if distance > POST_SIMILARITY_THRESHOLD {
            continue;
        }

        let post = post::table
            .find(post_signature.post_id)
            .select(Post::as_select())
            .first(&mut conn)?;
        similar_posts.push(SimilarPostInfo {
            distance,
            post: PostInfo::new(&mut conn, post, client_id)?,
        });
    }

    Ok(ReverseSearchInfo {
        exact_post: None,
        similar_posts,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewPostInfo {
    tags: Option<Vec<String>>,
    safety: PostSafety,
    source: Option<String>,
    relations: Option<Vec<i32>>,
    flags: Option<Vec<String>>, // TODO
    anonymous: Option<bool>,
    content_token: String,
}

fn create_post(auth_result: AuthResult, post_info: NewPostInfo) -> Result<PostInfo, api::Error> {
    let required_rank = match post_info.anonymous.unwrap_or(false) {
        true => config::privileges().post_create_anonymous,
        false => config::privileges().post_create_identified,
    };
    let client = auth_result?;
    api::verify_privilege(client.as_ref(), required_rank)?;

    let (_uuid, extension) = post_info.content_token.split_once('.').unwrap();
    let content_type = MimeType::from_extension(extension)?;
    let post_type = PostType::from(content_type);
    if post_type != PostType::Image {
        panic!("Unsupported post type!") // TODO
    }

    let temp_path = filesystem::temporary_upload_filepath(&post_info.content_token);
    let image_size = std::fs::metadata(&temp_path)?.len();
    let image = image::open(&temp_path)?;
    let image_checksum = content::image_checksum(&image);

    let client_id = client.map(|user| user.id);
    let new_post = NewPost {
        user_id: client_id,
        file_size: image_size as i64,
        width: image.width() as i32,
        height: image.height() as i32,
        safety: post_info.safety,
        type_: post_type,
        mime_type: content_type,
        checksum: &image_checksum,
        source: post_info.source.as_deref(),
    };

    let mut conn = crate::establish_connection()?;
    let post = diesel::insert_into(post::table)
        .values(&new_post)
        .returning(Post::as_returning())
        .get_result(&mut conn)?;

    // Add tags: TODO

    // Add relations
    let mut new_relations = Vec::new();
    for related_post_id in post_info.relations.unwrap_or_default().into_iter() {
        new_relations.push(NewPostRelation {
            parent_id: post.id,
            child_id: related_post_id,
        });
        new_relations.push(NewPostRelation {
            parent_id: related_post_id,
            child_id: post.id,
        })
    }
    diesel::insert_into(post_relation::table)
        .values(&new_relations)
        .execute(&mut conn)?;

    // Generate image signature
    let image_signature = signature::compute_signature(&image);
    let new_post_signature = NewPostSignature {
        post_id: post.id,
        signature: &image_signature,
        words: &signature::generate_indexes(&image_signature),
    };
    diesel::insert_into(post_signature::table)
        .values(&new_post_signature)
        .execute(&mut conn)?;

    // Move content to permanent location
    let posts_folder = filesystem::posts_directory();
    if !posts_folder.exists() {
        std::fs::create_dir(&posts_folder)?;
    }
    std::fs::rename(temp_path, content::post_content_path(post.id, post.mime_type))?;

    // Generate thumbnail
    let thumbnail_folder = filesystem::generated_thumbnails_directory();
    if !thumbnail_folder.exists() {
        std::fs::create_dir(&thumbnail_folder)?;
    }
    let thumbnail = image.resize_to_fill(
        config::get().thumbnails.post_width,
        config::get().thumbnails.post_height,
        image::imageops::FilterType::Nearest,
    );
    thumbnail.to_rgb8().save(content::post_thumbnail_path(post.id))?;

    PostInfo::new(&mut conn, post, client_id).map_err(api::Error::from)
}

fn delete_post(post_id: i32, auth_result: AuthResult, client_version: api::ResourceVersion) -> Result<(), api::Error> {
    let client = auth_result?;
    api::verify_privilege(client.as_ref(), config::privileges().post_delete)?;

    let mut conn = crate::establish_connection()?;
    let post = post::table.find(post_id).select(Post::as_select()).first(&mut conn)?;
    api::verify_version(post.last_edit_time, client_version)?;

    post.delete(&mut conn)?;

    Ok(())
}
