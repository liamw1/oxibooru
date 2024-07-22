use crate::api::{ApiResult, AuthResult, PagedQuery, PagedResponse, ResourceQuery};
use crate::auth::content;
use crate::image::signature;
use crate::model::enums::{MimeType, PostSafety, PostType};
use crate::model::post::{NewPost, NewPostRelation, NewPostSignature, Post, PostSignature};
use crate::resource::post::{FieldTable, PostInfo};
use crate::schema::{post, post_relation, post_signature};
use crate::{api, config, filesystem, resource, search};
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
        .and(api::optional_query())
        .map(get_post)
        .map(api::Reply::from);
    let get_post_neighbors = warp::get()
        .and(warp::path!("post" / i32 / "around"))
        .and(api::auth())
        .and(api::optional_query())
        .map(get_post_neighbors)
        .map(api::Reply::from);
    let reverse_search = warp::post()
        .and(warp::path!("posts" / "reverse-search"))
        .and(api::auth())
        .and(api::optional_query())
        .and(warp::body::json())
        .map(reverse_search)
        .map(api::Reply::from);
    let post_post = warp::post()
        .and(warp::path!("posts"))
        .and(api::auth())
        .and(api::optional_query())
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

type PagedPostInfo = PagedResponse<PostInfo>;

const MAX_POSTS_PER_PAGE: i64 = 50;
const POST_SIMILARITY_THRESHOLD: f64 = 0.4;

fn create_field_table(fields: Option<&str>) -> Result<FieldTable<bool>, Box<dyn std::error::Error>> {
    fields
        .map(resource::post::Field::create_table)
        .transpose()
        .map(|opt_table| opt_table.unwrap_or(FieldTable::filled(true)))
        .map_err(Box::from)
}

fn list_posts(auth: AuthResult, query: PagedQuery) -> ApiResult<PagedPostInfo> {
    let _timer = crate::util::Timer::new("list_posts");

    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().post_list)?;

    let client_id = client.map(|user| user.id);
    let offset = query.offset.unwrap_or(0);
    let limit = std::cmp::min(query.limit, MAX_POSTS_PER_PAGE);
    let fields = create_field_table(query.fields())?;

    let sql_query = search::post::build_query(client_id, query.criteria())?;
    println!("SQL Query: {}\n", diesel::debug_query(&sql_query).to_string());

    let mut conn = crate::establish_connection()?;

    // Selecting by Post for now, but would be more efficient to select by ids for large databases
    let posts: Vec<Post> = sql_query.select(Post::as_select()).load(&mut conn)?;
    let total = posts.len() as i64;
    let selected_posts: Vec<Post> = posts.into_iter().skip(offset as usize).take(limit as usize).collect();

    Ok(PagedPostInfo {
        query: query.query.query,
        offset,
        limit,
        total,
        results: PostInfo::new_batch(&mut conn, client_id, selected_posts, &fields)?,
    })
}

fn get_post(post_id: i32, auth: AuthResult, query: ResourceQuery) -> ApiResult<PostInfo> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().post_view)?;

    let fields = create_field_table(query.fields())?;

    let mut conn = crate::establish_connection()?;
    let post = post::table.find(post_id).select(Post::as_select()).first(&mut conn)?;
    let client_id = client.map(|user| user.id);
    PostInfo::new(&mut conn, client_id, post, &fields).map_err(api::Error::from)
}

#[derive(Serialize)]
struct PostNeighbors {
    prev: Option<PostInfo>,
    next: Option<PostInfo>,
}

fn get_post_neighbors(post_id: i32, auth: AuthResult, query: ResourceQuery) -> ApiResult<PostNeighbors> {
    let _timer = crate::util::Timer::new("get_post_neighbors");

    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().post_list)?;

    let client_id = client.map(|user| user.id);
    let fields = create_field_table(query.fields())?;
    let sql_query = search::post::build_query(client_id, query.criteria())?;
    let mut conn = crate::establish_connection()?;

    let post_ids: Vec<i32> = sql_query.load(&mut conn)?;
    let post_index = post_ids.iter().position(|&id| id == post_id);

    let prev_post = post_index
        .and_then(|index| post_ids.get(index - 1))
        .map(|prev_post_id| {
            post::table
                .find(prev_post_id)
                .select(Post::as_select())
                .first(&mut conn)
                .optional()
        })
        .transpose()?
        .flatten();
    let prev = prev_post
        .map(|post| PostInfo::new(&mut conn, client_id, post, &fields))
        .transpose()?;

    let next_post = post_index
        .and_then(|index| post_ids.get(index + 1))
        .map(|next_post_id| {
            post::table
                .find(next_post_id)
                .select(Post::as_select())
                .first(&mut conn)
                .optional()
        })
        .transpose()?
        .flatten();
    let next = next_post
        .map(|post| PostInfo::new(&mut conn, client_id, post, &fields))
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

fn reverse_search(auth: AuthResult, query: ResourceQuery, token: ContentToken) -> ApiResult<ReverseSearchInfo> {
    let _timer = crate::util::Timer::new("reverse_search");

    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().post_reverse_search)?;

    let fields = create_field_table(query.fields())?;

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
                .map(|post| PostInfo::new(&mut conn, client_id, post, &fields))
                .transpose()?,
            similar_posts: Vec::new(),
        });
    }

    // Search for similar images
    let similar_signatures = PostSignature::find_similar(&mut conn, signature::generate_indexes(&image_signature))?;
    println!("Found {} similar signatures", similar_signatures.len());
    let mut similar_post_ids: Vec<_> = similar_signatures
        .into_iter()
        .filter_map(|post_signature| {
            let distance = signature::normalized_distance(&post_signature.signature, &image_signature);
            (distance < POST_SIMILARITY_THRESHOLD).then_some((post_signature.post_id, distance))
        })
        .collect();
    similar_post_ids.sort_by(|(_, dist_a), (_, dist_b)| dist_a.partial_cmp(dist_b).unwrap());
    let similar_posts: Vec<_> = similar_post_ids
        .iter()
        .map(|(post_id, _)| post::table.find(post_id).select(Post::as_select()).first(&mut conn))
        .collect::<QueryResult<_>>()?;
    Ok(ReverseSearchInfo {
        exact_post: None,
        similar_posts: PostInfo::new_batch(&mut conn, client_id, similar_posts, &fields)?
            .into_iter()
            .zip(similar_post_ids.into_iter().map(|(_, distance)| distance))
            .map(|(post, distance)| SimilarPostInfo { distance, post })
            .collect(),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewPostInfo {
    _tags: Option<Vec<String>>, // TODO
    safety: PostSafety,
    source: Option<String>,
    relations: Option<Vec<i32>>,
    _flags: Option<Vec<String>>, // TODO
    anonymous: Option<bool>,
    content_token: String,
}

fn create_post(auth: AuthResult, query: ResourceQuery, post_info: NewPostInfo) -> ApiResult<PostInfo> {
    let required_rank = match post_info.anonymous.unwrap_or(false) {
        true => config::privileges().post_create_anonymous,
        false => config::privileges().post_create_identified,
    };
    let client = auth?;
    api::verify_privilege(client.as_ref(), required_rank)?;

    let fields = create_field_table(query.fields())?;

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

    PostInfo::new(&mut conn, client_id, post, &fields).map_err(api::Error::from)
}

fn delete_post(post_id: i32, auth: AuthResult, client_version: api::ResourceVersion) -> ApiResult<()> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().post_delete)?;

    let mut conn = crate::establish_connection()?;
    let post = post::table.find(post_id).select(Post::as_select()).first(&mut conn)?;
    api::verify_version(post.last_edit_time, client_version)?;

    post.delete(&mut conn)?;

    Ok(())
}
