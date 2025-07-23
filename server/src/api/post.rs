use crate::api::{ApiResult, DeleteBody, MergeBody, PageParams, PagedResponse, RatingBody, ResourceParams};
use crate::auth::Client;
use crate::content::hash::PostHash;
use crate::content::thumbnail::{ThumbnailCategory, ThumbnailType};
use crate::content::upload::{MAX_UPLOAD_SIZE, PartName};
use crate::content::{Content, FileContents, JsonOrMultipart, signature, upload};
use crate::filesystem::Directory;
use crate::model::comment::NewComment;
use crate::model::enums::{PostFlag, PostFlags, PostSafety, PostType, ResourceType, Score};
use crate::model::pool::PoolPost;
use crate::model::post::{
    CompressedSignature, NewPost, NewPostFeature, NewPostSignature, Post, PostFavorite, PostRelation, PostScore,
    PostSignature, PostTag, SignatureIndexes,
};
use crate::resource::post::{Note, PostInfo};
use crate::schema::{
    comment, pool_post, post, post_favorite, post_feature, post_relation, post_score, post_signature, post_statistics,
    post_tag,
};
use crate::search::post::QueryBuilder;
use crate::string::SmallString;
use crate::time::DateTime;
use crate::{api, config, db, filesystem, resource, update};
use axum::extract::{DefaultBodyLimit, Extension, Path, Query};
use axum::{Json, Router, routing};
use diesel::dsl::exists;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::LazyLock;
use tokio::sync::Mutex as AsyncMutex;
use tracing::info;
use url::Url;

pub fn routes() -> Router {
    Router::new()
        .route("/posts", routing::get(list).post(create_handler))
        .route(
            "/post/{id}",
            routing::get(get)
                .put(update_handler)
                .route_layer(DefaultBodyLimit::max(MAX_UPLOAD_SIZE))
                .delete(delete),
        )
        .route("/post/{id}/around", routing::get(get_neighbors))
        .route("/featured-post", routing::get(get_featured).post(feature))
        .route(
            "/posts/reverse-search",
            routing::post(reverse_search_handler).route_layer(DefaultBodyLimit::max(MAX_UPLOAD_SIZE)),
        )
        .route("/post-merge", routing::post(merge))
        .route("/post/{id}/favorite", routing::post(favorite).delete(unfavorite))
        .route("/post/{id}/score", routing::put(rate))
}

const MAX_POSTS_PER_PAGE: i64 = 1000;

static POST_TAG_MUTEX: LazyLock<AsyncMutex<()>> = LazyLock::new(|| AsyncMutex::new(()));

/// Runs an `update` that may add `tags` to a post as a transaction.
///
/// Tagging multiple posts simultaneously can cause issues if two updates share tags.
/// If two updates share a new tag, a uniqueness violation can occur.
/// If two updates share an existing tag, a deadlock can occur due to statistics updating.
/// Therefore, we lock an asynchronous mutex whenever updating post tags. This is a bit
/// more pessimistic than necessary, as parallel updates are safe if the sets of tags are
/// disjoint. However, allowing disjoint tagging introduces additional complexity so it
/// isn't being done as of now.
async fn tagging_update<T, F>(tags: Option<&[SmallString]>, update: F) -> ApiResult<T>
where
    F: FnOnce(&mut db::Connection) -> ApiResult<T>,
{
    let _lock;
    if tags.is_some() {
        _lock = POST_TAG_MUTEX.lock().await;
    }

    // Get a fresh connection here so that connection isn't being held while waiting on lock
    db::get_connection()?.transaction(update)
}

async fn list(
    Extension(client): Extension<Client>,
    Query(params): Query<PageParams>,
) -> ApiResult<Json<PagedResponse<PostInfo>>> {
    api::verify_privilege(client, config::privileges().post_list)?;

    let offset = params.offset.unwrap_or(0);
    let limit = std::cmp::min(params.limit.get(), MAX_POSTS_PER_PAGE);
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    db::get_connection()?.transaction(|conn| {
        let mut query_builder = QueryBuilder::new(client, params.criteria())?;
        query_builder.set_offset_and_limit(offset, limit);

        let total = query_builder.count(conn)?;
        let selected_posts = query_builder.load(conn)?;
        Ok(Json(PagedResponse {
            query: params.into_query(),
            offset,
            limit,
            total,
            results: PostInfo::new_batch_from_ids(conn, client, selected_posts, &fields)?,
        }))
    })
}

async fn get(
    Extension(client): Extension<Client>,
    Path(post_id): Path<i64>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<PostInfo>> {
    api::verify_privilege(client, config::privileges().post_view)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    db::get_connection()?.transaction(|conn| {
        let post_exists: bool = diesel::select(exists(post::table.find(post_id))).get_result(conn)?;
        if !post_exists {
            return Err(api::Error::NotFound(ResourceType::Post));
        }
        PostInfo::new_from_id(conn, client, post_id, &fields)
            .map(Json)
            .map_err(api::Error::from)
    })
}

#[derive(Serialize)]
struct PostNeighbors {
    prev: Option<PostInfo>,
    next: Option<PostInfo>,
}

async fn get_neighbors(
    Extension(client): Extension<Client>,
    Path(post_id): Path<i64>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<PostNeighbors>> {
    api::verify_privilege(client, config::privileges().post_list)?;

    let create_post_neighbors = |mut neighbors: Vec<PostInfo>, has_previous_post: bool| {
        let (prev, next) = match (neighbors.pop(), neighbors.pop()) {
            (Some(second), Some(first)) => (Some(first), Some(second)),
            (Some(first), None) if has_previous_post => (Some(first), None),
            (Some(first), None) => (None, Some(first)),
            (None, Some(_)) => unreachable!(),
            (None, None) => (None, None),
        };
        Json(PostNeighbors { prev, next })
    };

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let mut query_builder = QueryBuilder::new(client, params.criteria())?;
    db::get_connection()?.transaction(|conn| {
        const INITIAL_LIMIT: i64 = 100;
        const LIMIT_GROWTH: i64 = 8;

        // Handle special cases first
        if query_builder.criteria().has_random_sort() {
            query_builder.set_offset_and_limit(0, 2);
            let post_ids = query_builder.load_with(conn, |query| query.filter(post::id.ne(post_id)))?;
            let post_infos = PostInfo::new_batch_from_ids(conn, client, post_ids, &fields)?;
            return Ok(create_post_neighbors(post_infos, true));
        }
        if !query_builder.criteria().has_filter() && !query_builder.criteria().has_sort() {
            // Optimized neighbor retrieval for simplest use case
            let previous_post = post::table
                .select(Post::as_select())
                .filter(post::id.gt(post_id))
                .order_by(post::id.asc())
                .first(conn)
                .optional()?;
            let next_post = post::table
                .select(Post::as_select())
                .filter(post::id.lt(post_id))
                .order_by(post::id.desc())
                .first(conn)
                .optional()?;

            let has_previous_post = previous_post.is_some();
            let posts = previous_post.into_iter().chain(next_post).collect();
            let post_infos = PostInfo::new_batch(conn, client, posts, &fields)?;
            return Ok(create_post_neighbors(post_infos, has_previous_post));
        }

        // Search for neighbors using exponentially increasing limit
        let mut offset = 0;
        let mut limit = INITIAL_LIMIT;
        let (prev_post_id, next_post_id) = loop {
            query_builder.set_offset_and_limit(offset, limit);
            let post_id_batch = query_builder.load(conn)?;

            let post_index = post_id_batch.iter().position(|&id| id == post_id);
            if post_id_batch.len() < usize::try_from(limit).unwrap_or(usize::MAX)
                || post_id_batch.len() == usize::MAX
                || (post_index.is_some() && post_index != Some(post_id_batch.len().saturating_sub(1)))
            {
                let prev_post_id = post_index
                    .and_then(|index| index.checked_sub(1))
                    .and_then(|prev_index| post_id_batch.get(prev_index))
                    .copied();
                let next_post_id = post_index
                    .and_then(|index| index.checked_add(1))
                    .and_then(|next_index| post_id_batch.get(next_index))
                    .copied();
                break (prev_post_id, next_post_id);
            }

            offset += limit - 2;
            limit = limit.saturating_mul(LIMIT_GROWTH);
        };

        let post_ids = prev_post_id.into_iter().chain(next_post_id).collect();
        let post_infos = PostInfo::new_batch_from_ids(conn, client, post_ids, &fields)?;
        Ok(create_post_neighbors(post_infos, prev_post_id.is_some()))
    })
}

async fn get_featured(
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<Option<PostInfo>>> {
    api::verify_privilege(client, config::privileges().post_view_featured)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    db::get_connection()?.transaction(|conn| {
        let featured_post_id: Option<i64> = post_feature::table
            .select(post_feature::post_id)
            .order_by(post_feature::time.desc())
            .first(conn)
            .optional()?;

        featured_post_id
            .map(|post_id| PostInfo::new_from_id(conn, client, post_id, &fields))
            .transpose()
            .map(Json)
            .map_err(api::Error::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FeatureBody {
    id: i64,
}

async fn feature(
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<FeatureBody>,
) -> ApiResult<Json<PostInfo>> {
    api::verify_privilege(client, config::privileges().post_feature)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let user_id = client.id.ok_or(api::Error::NotLoggedIn)?;
    let new_post_feature = NewPostFeature {
        post_id: body.id,
        user_id,
        time: DateTime::now(),
    };

    let mut conn = db::get_connection()?;
    new_post_feature.insert_into(post_feature::table).execute(&mut conn)?;
    conn.transaction(|conn| PostInfo::new_from_id(conn, client, body.id, &fields))
        .map(Json)
        .map_err(api::Error::from)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct ReverseSearchBody {
    #[serde(skip_deserializing)]
    content: Option<FileContents>,
    content_token: Option<String>,
    content_url: Option<Url>,
}

#[derive(Serialize)]
struct SimilarPost {
    distance: f64,
    post: PostInfo,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReverseSearchResponse {
    exact_post: Option<PostInfo>,
    similar_posts: Vec<SimilarPost>,
}

async fn reverse_search(
    client: Client,
    params: ResourceParams,
    body: ReverseSearchBody,
) -> ApiResult<Json<ReverseSearchResponse>> {
    api::verify_privilege(client, config::privileges().post_reverse_search)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let content = Content::new(body.content, body.content_token, body.content_url)
        .ok_or(api::Error::MissingContent(ResourceType::Post))?;
    let content_properties = content.compute_properties().await?;
    db::get_connection()?
        .transaction(|conn| {
            let _timer = crate::time::Timer::new("reverse search");

            // Check for exact match
            let exact_post = post::table
                .filter(post::checksum.eq(content_properties.checksum))
                .first(conn)
                .optional()?;
            if exact_post.is_some() {
                return Ok(ReverseSearchResponse {
                    exact_post: exact_post
                        .map(|post_id| PostInfo::new(conn, client, post_id, &fields))
                        .transpose()?,
                    similar_posts: Vec::new(),
                });
            }

            // Search for similar images candidates
            let similar_signature_candidates = PostSignature::find_similar_candidates(
                conn,
                signature::generate_indexes(&content_properties.signature),
            )?;
            info!("Found {} similar signatures", similar_signature_candidates.len());

            // Filter candidates based on similarity score
            let content_signature_cache = signature::cache(&content_properties.signature);
            let mut similar_signatures: Vec<_> = similar_signature_candidates
                .into_iter()
                .filter_map(|post_signature| {
                    let distance = signature::distance(&content_signature_cache, &post_signature.signature);
                    let distance_threshold = 1.0 - config::get().post_similarity_threshold;
                    (distance < distance_threshold).then_some((post_signature.post_id, distance))
                })
                .collect();
            if similar_signatures.is_empty() {
                return Ok(ReverseSearchResponse {
                    exact_post: None,
                    similar_posts: Vec::new(),
                });
            }

            similar_signatures.sort_unstable_by(|(_, dist_a), (_, dist_b)| dist_a.partial_cmp(dist_b).unwrap());

            let (post_ids, distances): (Vec<_>, Vec<_>) = similar_signatures.into_iter().unzip();
            Ok(ReverseSearchResponse {
                exact_post: None,
                similar_posts: PostInfo::new_batch_from_ids(conn, client, post_ids, &fields)?
                    .into_iter()
                    .zip(distances)
                    .map(|(post, distance)| SimilarPost { distance, post })
                    .collect(),
            })
        })
        .map(Json)
}

async fn reverse_search_handler(
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
    body: JsonOrMultipart<ReverseSearchBody>,
) -> ApiResult<Json<ReverseSearchResponse>> {
    api::verify_privilege(client, config::privileges().post_reverse_search)?;

    match body {
        JsonOrMultipart::Json(payload) => reverse_search(client, params, payload).await,
        JsonOrMultipart::Multipart(payload) => {
            let decoded_body = upload::extract(payload, [PartName::Content]).await?;
            let reverse_search_body = if let [Some(content)] = decoded_body.files {
                ReverseSearchBody {
                    content: Some(content),
                    content_token: None,
                    content_url: None,
                }
            } else if let Some(metadata) = decoded_body.metadata {
                serde_json::from_slice(&metadata)?
            } else {
                return Err(api::Error::MissingFormData);
            };
            reverse_search(client, params, reverse_search_body).await
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct CreateBody {
    safety: PostSafety,
    #[serde(skip_deserializing)]
    content: Option<FileContents>,
    content_token: Option<String>,
    content_url: Option<Url>,
    #[serde(skip_deserializing)]
    thumbnail: Option<FileContents>,
    thumbnail_token: Option<String>,
    thumbnail_url: Option<Url>,
    source: Option<String>,
    description: Option<String>,
    relations: Option<Vec<i64>>,
    anonymous: Option<bool>,
    tags: Option<Vec<SmallString>>,
    notes: Option<Vec<Note>>,
    flags: Option<Vec<PostFlag>>,
}

async fn create(client: Client, params: ResourceParams, body: CreateBody) -> ApiResult<Json<PostInfo>> {
    let required_rank = match body.anonymous.unwrap_or(false) {
        true => config::privileges().post_create_anonymous,
        false => config::privileges().post_create_identified,
    };
    api::verify_privilege(client, required_rank)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let content = Content::new(body.content, body.content_token, body.content_url)
        .ok_or(api::Error::MissingContent(ResourceType::Post))?;
    let content_properties = content.get_or_compute_properties().await?;

    let custom_thumbnail = match Content::new(body.thumbnail, body.thumbnail_token, body.thumbnail_url) {
        Some(content) => Some(content.thumbnail(ThumbnailType::Post).await?),
        None => None,
    };
    let flags = content_properties.flags | PostFlags::from_slice(&body.flags.unwrap_or_default());

    let new_post = NewPost {
        user_id: client.id,
        file_size: content_properties.file_size as i64,
        width: content_properties.width as i32,
        height: content_properties.height as i32,
        safety: body.safety,
        type_: PostType::from(content_properties.mime_type),
        mime_type: content_properties.mime_type,
        checksum: &content_properties.checksum,
        checksum_md5: &content_properties.md5_checksum,
        flags,
        source: body.source.as_deref().unwrap_or(""),
        description: body.description.as_deref().unwrap_or(""),
    };

    let post = tagging_update(body.tags.as_deref(), |conn| {
        // We do this before post insertion so that the post sequence isn't incremented if it fails
        let tag_ids = body
            .tags
            .as_deref()
            .map(|names| update::tag::get_or_create_tag_ids(conn, client, names, false))
            .transpose()?;

        let post: Post = new_post.insert_into(post::table).get_result(conn)?;
        let post_hash = PostHash::new(post.id);

        // Add tags, relations, and notes
        if let Some(tags) = tag_ids {
            update::post::add_tags(conn, post.id, tags)?;
        }
        if let Some(relations) = body.relations {
            update::post::create_relations(conn, post.id, relations)?;
        }
        if let Some(notes) = body.notes {
            update::post::add_notes(conn, post.id, notes)?;
        }

        NewPostSignature {
            post_id: post.id,
            signature: content_properties.signature.into(),
            words: signature::generate_indexes(&content_properties.signature).into(),
        }
        .insert_into(post_signature::table)
        .execute(conn)?;

        // Move content to permanent location
        let temp_path = filesystem::temporary_upload_filepath(&content_properties.token);
        filesystem::create_dir(Directory::Posts)?;
        filesystem::move_file(&temp_path, &post_hash.content_path(content_properties.mime_type))?;

        // Create thumbnails
        if let Some(thumbnail) = custom_thumbnail {
            api::verify_privilege(client, config::privileges().post_edit_thumbnail)?;
            update::post::thumbnail(conn, &post_hash, thumbnail, ThumbnailCategory::Custom)?;
        }
        update::post::thumbnail(conn, &post_hash, content_properties.thumbnail, ThumbnailCategory::Generated)?;
        Ok::<_, api::Error>(post)
    })
    .await?;
    db::get_connection()?
        .transaction(|conn| PostInfo::new(conn, client, post, &fields))
        .map(Json)
        .map_err(api::Error::from)
}

async fn create_handler(
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
    body: JsonOrMultipart<CreateBody>,
) -> ApiResult<Json<PostInfo>> {
    match body {
        JsonOrMultipart::Json(payload) => create(client, params, payload).await,
        JsonOrMultipart::Multipart(payload) => {
            let decoded_body = upload::extract(payload, [PartName::Content, PartName::Thumbnail]).await?;
            let metadata = decoded_body.metadata.ok_or(api::Error::MissingMetadata)?;
            let mut new_post: CreateBody = serde_json::from_slice(&metadata)?;
            let [content, thumbnail] = decoded_body.files;

            new_post.content = content;
            new_post.thumbnail = thumbnail;
            create(client, params, new_post).await
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct PostMergeBody {
    #[serde(flatten)]
    post_info: MergeBody<i64>,
    replace_content: bool,
}

async fn merge(
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<PostMergeBody>,
) -> ApiResult<Json<PostInfo>> {
    api::verify_privilege(client, config::privileges().post_merge)?;

    let remove_id = body.post_info.remove;
    let merge_to_id = body.post_info.merge_to;
    if remove_id == merge_to_id {
        return Err(api::Error::SelfMerge(ResourceType::Post));
    }
    let remove_hash = PostHash::new(remove_id);
    let merge_to_hash = PostHash::new(merge_to_id);

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    tagging_update(Some(&[]), |conn| {
        let remove_post: Post = post::table.find(remove_id).first(conn)?;
        let merge_to_post: Post = post::table.find(merge_to_id).first(conn)?;
        api::verify_version(remove_post.last_edit_time, body.post_info.remove_version)?;
        api::verify_version(merge_to_post.last_edit_time, body.post_info.merge_to_version)?;

        // Merge relations
        let involved_relations: Vec<PostRelation> = post_relation::table
            .filter(post_relation::parent_id.eq(remove_id))
            .or_filter(post_relation::child_id.eq(remove_id))
            .or_filter(post_relation::parent_id.eq(merge_to_id))
            .or_filter(post_relation::child_id.eq(merge_to_id))
            .load(conn)?;
        let merged_relations: HashSet<_> = involved_relations
            .iter()
            .copied()
            .map(|mut relation| {
                if relation.parent_id == remove_id {
                    relation.parent_id = merge_to_id
                } else if relation.child_id == remove_id {
                    relation.child_id = merge_to_id
                }
                relation
            })
            .filter(|relation| relation.parent_id != relation.child_id)
            .collect();
        diesel::delete(post_relation::table)
            .filter(post_relation::parent_id.eq(merge_to_id))
            .or_filter(post_relation::child_id.eq(merge_to_id))
            .execute(conn)?;
        let merged_relations: Vec<_> = merged_relations.into_iter().collect();
        merged_relations.insert_into(post_relation::table).execute(conn)?;

        // Merge tags
        let merge_to_tags = post_tag::table
            .select(post_tag::tag_id)
            .filter(post_tag::post_id.eq(merge_to_id))
            .into_boxed();
        let new_tags: Vec<_> = post_tag::table
            .select(post_tag::tag_id)
            .filter(post_tag::post_id.eq(remove_id))
            .filter(post_tag::tag_id.ne_all(merge_to_tags))
            .load(conn)?
            .into_iter()
            .map(|tag_id| PostTag {
                post_id: merge_to_id,
                tag_id,
            })
            .collect();
        new_tags.insert_into(post_tag::table).execute(conn)?;

        // Merge pools
        let merge_to_pools = pool_post::table
            .select(pool_post::pool_id)
            .filter(pool_post::post_id.eq(merge_to_id))
            .into_boxed();
        let new_pools: Vec<_> = pool_post::table
            .select((pool_post::pool_id, pool_post::order))
            .filter(pool_post::post_id.eq(remove_id))
            .filter(pool_post::pool_id.ne_all(merge_to_pools))
            .load(conn)?
            .into_iter()
            .map(|(pool_id, order)| PoolPost {
                pool_id,
                post_id: merge_to_id,
                order,
            })
            .collect();
        new_pools.insert_into(pool_post::table).execute(conn)?;

        // Merge scores
        let merge_to_scores = post_score::table
            .select(post_score::user_id)
            .filter(post_score::post_id.eq(merge_to_id))
            .into_boxed();
        let new_scores: Vec<_> = post_score::table
            .select((post_score::user_id, post_score::score, post_score::time))
            .filter(post_score::post_id.eq(remove_id))
            .filter(post_score::user_id.ne_all(merge_to_scores))
            .load(conn)?
            .into_iter()
            .map(|(user_id, score, time)| PostScore {
                post_id: merge_to_id,
                user_id,
                score,
                time,
            })
            .collect();
        new_scores.insert_into(post_score::table).execute(conn)?;

        // Merge favorites
        let merge_to_favorites = post_favorite::table
            .select(post_favorite::user_id)
            .filter(post_favorite::post_id.eq(merge_to_id))
            .into_boxed();
        let new_favorites: Vec<_> = post_favorite::table
            .select((post_favorite::user_id, post_favorite::time))
            .filter(post_favorite::post_id.eq(remove_id))
            .filter(post_favorite::user_id.ne_all(merge_to_favorites))
            .load(conn)?
            .into_iter()
            .map(|(user_id, time)| PostFavorite {
                post_id: merge_to_id,
                user_id,
                time,
            })
            .collect();
        new_favorites.insert_into(post_favorite::table).execute(conn)?;

        // Merge features
        let new_features: Vec<_> = diesel::delete(post_feature::table.filter(post_feature::post_id.eq(remove_id)))
            .returning((post_feature::user_id, post_feature::time))
            .get_results(conn)?
            .into_iter()
            .map(|(user_id, time)| NewPostFeature {
                post_id: merge_to_id,
                user_id,
                time,
            })
            .collect();
        new_features.insert_into(post_feature::table).execute(conn)?;

        // Merge comments
        let removed_comments: Vec<(_, String, _)> =
            diesel::delete(comment::table.filter(comment::post_id.eq(remove_id)))
                .returning((comment::user_id, comment::text, comment::creation_time))
                .get_results(conn)?;
        let new_comments: Vec<_> = removed_comments
            .iter()
            .map(|(user_id, text, creation_time)| NewComment {
                user_id: *user_id,
                post_id: merge_to_id,
                text,
                creation_time: *creation_time,
            })
            .collect();
        new_comments.insert_into(comment::table).execute(conn)?;

        // Merge descriptions
        let merged_description = merge_to_post.description.clone() + "\n\n" + &remove_post.description;
        diesel::update(post::table.find(merge_to_id))
            .set(post::description.eq(merged_description.trim()))
            .execute(conn)?;

        // If replacing content, update post signature. This needs to be done before deletion because post signatures cascade
        if body.replace_content && !cfg!(test) {
            let (signature, indexes): (CompressedSignature, SignatureIndexes) = post_signature::table
                .find(remove_id)
                .select((post_signature::signature, post_signature::words))
                .first(conn)?;
            diesel::update(post_signature::table.find(merge_to_id))
                .set((post_signature::signature.eq(signature), post_signature::words.eq(indexes)))
                .execute(conn)?;
        }

        diesel::delete(post::table.find(remove_id)).execute(conn)?;

        if body.replace_content {
            if !cfg!(test) {
                filesystem::swap_posts(&remove_hash, remove_post.mime_type, &merge_to_hash, merge_to_post.mime_type)?;
            }

            // If replacing content, update metadata. This needs to be done after deletion because checksum has UNIQUE constraint
            diesel::update(post::table.find(merge_to_post.id))
                .set((
                    post::file_size.eq(remove_post.file_size),
                    post::width.eq(remove_post.width),
                    post::height.eq(remove_post.height),
                    post::type_.eq(remove_post.type_),
                    post::mime_type.eq(remove_post.mime_type),
                    post::checksum.eq(remove_post.checksum),
                    post::checksum_md5.eq(remove_post.checksum_md5),
                    post::flags.eq(remove_post.flags),
                    post::source.eq(remove_post.source),
                    post::generated_thumbnail_size.eq(remove_post.generated_thumbnail_size),
                    post::custom_thumbnail_size.eq(remove_post.custom_thumbnail_size),
                ))
                .execute(conn)?;
        }

        if config::get().delete_source_files && !cfg!(test) {
            let deleted_content_type = if body.replace_content {
                merge_to_post.mime_type
            } else {
                remove_post.mime_type
            };
            filesystem::delete_post(&remove_hash, deleted_content_type)?;
        }
        update::post::last_edit_time(conn, merge_to_id)?;
        Ok(())
    })
    .await?;
    db::get_connection()?
        .transaction(|conn| PostInfo::new_from_id(conn, client, merge_to_id, &fields))
        .map(Json)
        .map_err(api::Error::from)
}

async fn favorite(
    Extension(client): Extension<Client>,
    Path(post_id): Path<i64>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<PostInfo>> {
    api::verify_privilege(client, config::privileges().post_favorite)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let user_id = client.id.ok_or(api::Error::NotLoggedIn)?;
    let new_post_favorite = PostFavorite {
        post_id,
        user_id,
        time: DateTime::now(),
    };

    let mut conn = db::get_connection()?;
    conn.transaction(|conn| {
        diesel::delete(post_favorite::table.find((post_id, user_id))).execute(conn)?;
        new_post_favorite
            .insert_into(post_favorite::table)
            .execute(conn)
            .map_err(api::Error::from)
    })?;
    conn.transaction(|conn| PostInfo::new_from_id(conn, client, post_id, &fields))
        .map(Json)
        .map_err(api::Error::from)
}

async fn rate(
    Extension(client): Extension<Client>,
    Path(post_id): Path<i64>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<RatingBody>,
) -> ApiResult<Json<PostInfo>> {
    api::verify_privilege(client, config::privileges().post_score)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let user_id = client.id.ok_or(api::Error::NotLoggedIn)?;

    let mut conn = db::get_connection()?;
    conn.transaction(|conn| {
        diesel::delete(post_score::table.find((post_id, user_id))).execute(conn)?;

        if let Ok(score) = Score::try_from(*body) {
            PostScore {
                post_id,
                user_id,
                score,
                time: DateTime::now(),
            }
            .insert_into(post_score::table)
            .execute(conn)?;
        }
        Ok::<_, api::Error>(())
    })?;
    conn.transaction(|conn| PostInfo::new_from_id(conn, client, post_id, &fields))
        .map(Json)
        .map_err(api::Error::from)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct UpdateBody {
    version: DateTime,
    safety: Option<PostSafety>,
    source: Option<String>,
    description: Option<String>,
    relations: Option<Vec<i64>>,
    tags: Option<Vec<SmallString>>,
    notes: Option<Vec<Note>>,
    flags: Option<Vec<PostFlag>>,
    #[serde(skip_deserializing)]
    content: Option<FileContents>,
    content_token: Option<String>,
    content_url: Option<Url>,
    #[serde(skip_deserializing)]
    thumbnail: Option<FileContents>,
    thumbnail_token: Option<String>,
    thumbnail_url: Option<Url>,
}

async fn update(client: Client, post_id: i64, params: ResourceParams, body: UpdateBody) -> ApiResult<Json<PostInfo>> {
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let post_hash = PostHash::new(post_id);

    let new_content = match Content::new(body.content, body.content_token, body.content_url) {
        Some(content) => Some(content.get_or_compute_properties().await?),
        None => None,
    };
    let custom_thumbnail = match Content::new(body.thumbnail, body.thumbnail_token, body.thumbnail_url) {
        Some(content) => Some(content.thumbnail(ThumbnailType::Post).await?),
        None => None,
    };

    tagging_update(body.tags.as_deref(), |conn| {
        let post_version = post::table.find(post_id).select(post::last_edit_time).first(conn)?;
        api::verify_version(post_version, body.version)?;

        if let Some(safety) = body.safety {
            api::verify_privilege(client, config::privileges().post_edit_safety)?;

            diesel::update(post::table.find(post_id))
                .set(post::safety.eq(safety))
                .execute(conn)?;
        }
        if let Some(source) = body.source {
            api::verify_privilege(client, config::privileges().post_edit_source)?;

            diesel::update(post::table.find(post_id))
                .set(post::source.eq(source))
                .execute(conn)?;
        }
        if let Some(description) = body.description {
            api::verify_privilege(client, config::privileges().post_edit_description)?;

            diesel::update(post::table.find(post_id))
                .set(post::description.eq(description))
                .execute(conn)?;
        }
        if let Some(relations) = body.relations {
            api::verify_privilege(client, config::privileges().post_edit_relation)?;

            update::post::delete_relations(conn, post_id)?;
            update::post::create_relations(conn, post_id, relations)?;
        }
        if let Some(tags) = body.tags.as_deref() {
            api::verify_privilege(client, config::privileges().post_edit_tag)?;

            let updated_tag_ids = update::tag::get_or_create_tag_ids(conn, client, tags, false)?;
            update::post::delete_tags(conn, post_id)?;
            update::post::add_tags(conn, post_id, updated_tag_ids)?;
        }
        if let Some(notes) = body.notes {
            api::verify_privilege(client, config::privileges().post_edit_note)?;

            update::post::delete_notes(conn, post_id)?;
            update::post::add_notes(conn, post_id, notes)?;
        }
        if let Some(flags) = body.flags {
            api::verify_privilege(client, config::privileges().post_edit_flag)?;

            let updated_flags = PostFlags::from_slice(&flags);
            diesel::update(post::table.find(post_id))
                .set(post::flags.eq(updated_flags))
                .execute(conn)?;
        }
        if let Some(content_properties) = new_content {
            api::verify_privilege(client, config::privileges().post_edit_content)?;

            let mut post: Post = post::table.find(post_id).first(conn)?;
            let old_mime_type = post.mime_type;

            // Update content metadata
            post.file_size = content_properties.file_size as i64;
            post.width = content_properties.width as i32;
            post.height = content_properties.height as i32;
            post.type_ = PostType::from(content_properties.mime_type);
            post.mime_type = content_properties.mime_type;
            post.checksum = content_properties.checksum;
            post.flags |= content_properties.flags;
            post.save_changes::<Post>(conn)?;

            // Update post signature
            let new_post_signature = NewPostSignature {
                post_id,
                signature: content_properties.signature.into(),
                words: signature::generate_indexes(&content_properties.signature).into(),
            };
            diesel::delete(post_signature::table.find(post_id)).execute(conn)?;
            new_post_signature.insert_into(post_signature::table).execute(conn)?;

            // Replace content
            let temp_path = filesystem::temporary_upload_filepath(&content_properties.token);
            filesystem::delete_content(&post_hash, old_mime_type)?;
            filesystem::move_file(&temp_path, &post_hash.content_path(content_properties.mime_type))?;

            // Replace generated thumbnail
            update::post::thumbnail(conn, &post_hash, content_properties.thumbnail, ThumbnailCategory::Generated)?;
        }
        if let Some(thumbnail) = custom_thumbnail {
            api::verify_privilege(client, config::privileges().post_edit_thumbnail)?;
            update::post::thumbnail(conn, &post_hash, thumbnail, ThumbnailCategory::Custom)?;
        }
        update::post::last_edit_time(conn, post_id)
    })
    .await?;
    db::get_connection()?
        .transaction(|conn| PostInfo::new_from_id(conn, client, post_id, &fields))
        .map(Json)
        .map_err(api::Error::from)
}

async fn update_handler(
    Extension(client): Extension<Client>,
    Path(post_id): Path<i64>,
    Query(params): Query<ResourceParams>,
    body: JsonOrMultipart<UpdateBody>,
) -> ApiResult<Json<PostInfo>> {
    match body {
        JsonOrMultipart::Json(payload) => update(client, post_id, params, payload).await,
        JsonOrMultipart::Multipart(payload) => {
            let decoded_body = upload::extract(payload, [PartName::Content, PartName::Thumbnail]).await?;
            let metadata = decoded_body.metadata.ok_or(api::Error::MissingMetadata)?;
            let mut post_update: UpdateBody = serde_json::from_slice(&metadata)?;
            let [content, thumbnail] = decoded_body.files;

            post_update.content = content;
            post_update.thumbnail = thumbnail;
            update(client, post_id, params, post_update).await
        }
    }
}

async fn delete(
    Extension(client): Extension<Client>,
    Path(post_id): Path<i64>,
    Json(client_version): Json<DeleteBody>,
) -> ApiResult<Json<()>> {
    api::verify_privilege(client, config::privileges().post_delete)?;

    let relation_count: i64 = post_statistics::table
        .find(post_id)
        .select(post_statistics::relation_count)
        .first(&mut db::get_connection()?)?;

    // Post relation cascade deletion can cause deadlocks when deleting related posts in quick
    // succession, so we lock an aysnchronous mutex when deleting if the post has any relations.
    static ANTI_DEADLOCK_MUTEX: LazyLock<AsyncMutex<()>> = LazyLock::new(|| AsyncMutex::new(()));
    let _lock;
    if relation_count > 0 {
        _lock = ANTI_DEADLOCK_MUTEX.lock().await;
    }

    let mut conn = db::get_connection()?;
    let mime_type = conn.transaction(|conn| {
        let (mime_type, post_version) = post::table
            .find(post_id)
            .select((post::mime_type, post::last_edit_time))
            .first(conn)?;
        api::verify_version(post_version, *client_version)?;

        diesel::delete(post::table.find(post_id)).execute(conn)?;
        Ok::<_, api::Error>(mime_type)
    })?;
    if config::get().delete_source_files {
        filesystem::delete_post(&PostHash::new(post_id), mime_type)?;
    }
    Ok(Json(()))
}

async fn unfavorite(
    Extension(client): Extension<Client>,
    Path(post_id): Path<i64>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<PostInfo>> {
    api::verify_privilege(client, config::privileges().post_favorite)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let user_id = client.id.ok_or(api::Error::NotLoggedIn)?;

    let mut conn = db::get_connection()?;
    diesel::delete(post_favorite::table.find((post_id, user_id))).execute(&mut conn)?;
    conn.transaction(|conn| PostInfo::new_from_id(conn, client, post_id, &fields))
        .map(Json)
        .map_err(api::Error::from)
}

#[cfg(test)]
mod test {
    use crate::api::ApiResult;
    use crate::model::post::Post;
    use crate::schema::{post, post_feature, post_statistics, tag, tag_name, user, user_statistics};
    use crate::search::post::Token;
    use crate::test::*;
    use crate::time::DateTime;
    use diesel::dsl::exists;
    use diesel::prelude::*;
    use serial_test::{parallel, serial};
    use strum::IntoEnumIterator;

    // Exclude fields that involve creation_time or last_edit_time
    const FIELDS: &str = "&fields=id,user,fileSize,canvasWidth,canvasHeight,safety,type,mimeType,checksum,checksumMd5,\
    flags,source,description,contentUrl,thumbnailUrl,tags,relations,pools,notes,score,ownScore,ownFavorite,tagCount,commentCount,\
    relationCount,noteCount,favoriteCount,featureCount,favoritedBy,hasCustomThumbnail";

    #[tokio::test]
    #[parallel]
    async fn list() -> ApiResult<()> {
        const QUERY: &str = "GET /posts/?query";
        const SORT: &str = "-sort:id&limit=40";
        verify_query(&format!("{QUERY}={SORT}{FIELDS}"), "post/list.json").await?;

        // Test sorts
        for token in Token::iter() {
            match token {
                Token::Id | Token::ContentChecksum | Token::NoteText | Token::Special => continue,
                _ => (),
            };
            let token_str: &'static str = token.into();
            let query = format!("{QUERY}=sort:{token_str} {SORT}&fields=id");
            let path = format!("post/list_{token_str}_sorted.json");
            verify_query(&query, &path).await?;
        }

        // Test filters
        verify_query(&format!("{QUERY}=-plant,sky,tagme {SORT}&fields=id"), "post/list_tag_filtered.json").await?;
        verify_query(&format!("{QUERY}=-pool:2 {SORT}&fields=id"), "post/list_pool_filtered.json").await?;
        verify_query(&format!("{QUERY}=fav:*user* {SORT}&fields=id"), "post/list_fav_filtered.json").await?;
        verify_query(&format!("{QUERY}=-comment:*user* {SORT}&fields=id"), "post/list_comment_filtered.json").await?;
        verify_query(&format!("{QUERY}=note-text:*fav* {SORT}&fields=id"), "post/list_note-text_filtered.json").await?;
        verify_query(&format!("{QUERY}=special:liked {SORT}&fields=id"), "post/list_liked_filtered.json").await?;
        verify_query(&format!("{QUERY}=special:disliked {SORT}&fields=id"), "post/list_disliked_filtered.json").await?;
        verify_query(&format!("{QUERY}=special:fav {SORT}&fields=id"), "post/list_special-fav_filtered.json").await?;
        verify_query(&format!("{QUERY}=special:tumbleweed {SORT}&fields=id"), "post/list_tumbleweed_filtered.json")
            .await
    }

    #[tokio::test]
    #[parallel]
    async fn get() -> ApiResult<()> {
        const POST_ID: i64 = 2;
        let get_last_edit_time = |conn: &mut PgConnection| -> QueryResult<DateTime> {
            post::table
                .select(post::last_edit_time)
                .filter(post::id.eq(POST_ID))
                .first(conn)
        };

        let mut conn = get_connection()?;
        let last_edit_time = get_last_edit_time(&mut conn)?;

        verify_query(&format!("GET /post/{POST_ID}/?{FIELDS}"), "post/get.json").await?;

        let new_last_edit_time = get_last_edit_time(&mut conn)?;
        assert_eq!(new_last_edit_time, last_edit_time);
        Ok(())
    }

    #[tokio::test]
    #[parallel]
    async fn get_neighbors() -> ApiResult<()> {
        const QUERY: &str = "around/?query=-sort:id";
        verify_query(&format!("GET /post/1/{QUERY}{FIELDS}"), "post/get_1_neighbors.json").await?;
        verify_query(&format!("GET /post/4/{QUERY}{FIELDS}"), "post/get_4_neighbors.json").await?;
        verify_query(&format!("GET /post/5/{QUERY}{FIELDS}"), "post/get_5_neighbors.json").await
    }

    #[tokio::test]
    #[parallel]
    async fn get_featured() -> ApiResult<()> {
        verify_query(&format!("GET /featured-post/?{FIELDS}"), "post/get_featured.json").await
    }

    #[tokio::test]
    #[serial]
    async fn feature() -> ApiResult<()> {
        const POST_ID: i64 = 4;
        let get_post_info = |conn: &mut PgConnection| -> QueryResult<(i64, DateTime)> {
            post::table
                .inner_join(post_statistics::table)
                .select((post_statistics::feature_count, post::last_edit_time))
                .filter(post::id.eq(POST_ID))
                .first(conn)
        };

        let mut conn = get_connection()?;
        let (feature_count, last_edit_time) = get_post_info(&mut conn)?;

        verify_query(&format!("POST /featured-post/?{FIELDS}"), "post/feature.json").await?;

        let (new_feature_count, new_last_edit_time) = get_post_info(&mut conn)?;
        assert_eq!(new_feature_count, feature_count + 1);
        assert_eq!(new_last_edit_time, last_edit_time);

        let last_feature_time: DateTime = post_feature::table
            .select(post_feature::time)
            .order_by(post_feature::time.desc())
            .first(&mut conn)?;
        diesel::delete(post_feature::table)
            .filter(post_feature::post_id.eq(POST_ID))
            .filter(post_feature::time.eq(last_feature_time))
            .execute(&mut conn)?;
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn merge() -> ApiResult<()> {
        const REMOVE_ID: i64 = 2;
        const MERGE_TO_ID: i64 = 1;
        let get_post = |conn: &mut PgConnection| -> QueryResult<Post> {
            post::table
                .select(Post::as_select())
                .filter(post::id.eq(MERGE_TO_ID))
                .first(conn)
        };

        let mut conn = get_connection()?;
        let post = get_post(&mut conn)?;

        verify_query(&format!("POST /post-merge/?{FIELDS}"), "post/merge.json").await?;

        let has_post: bool = diesel::select(exists(post::table.find(REMOVE_ID))).get_result(&mut conn)?;
        assert!(!has_post);

        let new_post = get_post(&mut conn)?;
        assert_eq!(new_post.user_id, post.user_id);
        assert_ne!(new_post.file_size, post.file_size);
        assert_ne!(new_post.width, post.width);
        assert_ne!(new_post.height, post.height);
        assert_eq!(new_post.safety, post.safety);
        assert_ne!(new_post.type_, post.type_);
        assert_ne!(new_post.mime_type, post.mime_type);
        assert_ne!(new_post.checksum, post.checksum);
        assert_ne!(new_post.checksum_md5, post.checksum_md5);
        assert_eq!(new_post.flags, post.flags);
        assert_ne!(new_post.source, post.source);
        assert_eq!(new_post.description, post.description);
        assert_eq!(new_post.creation_time, post.creation_time);
        assert!(new_post.last_edit_time > post.last_edit_time);
        Ok(reset_database())
    }

    #[tokio::test]
    #[serial]
    async fn favorite() -> ApiResult<()> {
        const POST_ID: i64 = 4;
        let get_post_info = |conn: &mut PgConnection| -> QueryResult<(i64, i64, DateTime)> {
            let (favorite_count, last_edit_time) = post::table
                .inner_join(post_statistics::table)
                .select((post_statistics::favorite_count, post::last_edit_time))
                .filter(post_statistics::post_id.eq(POST_ID))
                .first(conn)?;
            let admin_favorite_count = user::table
                .inner_join(user_statistics::table)
                .select(user_statistics::favorite_count)
                .filter(user::name.eq("administrator"))
                .first(conn)?;
            Ok((favorite_count, admin_favorite_count, last_edit_time))
        };

        let mut conn = get_connection()?;
        let (favorite_count, admin_favorite_count, last_edit_time) = get_post_info(&mut conn)?;

        verify_query(&format!("POST /post/{POST_ID}/favorite/?{FIELDS}"), "post/favorite.json").await?;

        let (new_favorite_count, new_admin_favorite_count, new_last_edit_time) = get_post_info(&mut conn)?;
        assert_eq!(new_favorite_count, favorite_count + 1);
        assert_eq!(new_admin_favorite_count, admin_favorite_count + 1);
        assert_eq!(new_last_edit_time, last_edit_time);

        verify_query(&format!("DELETE /post/{POST_ID}/favorite/?{FIELDS}"), "post/unfavorite.json").await?;

        let (new_favorite_count, new_admin_favorite_count, new_last_edit_time) = get_post_info(&mut conn)?;
        assert_eq!(new_favorite_count, favorite_count);
        assert_eq!(new_admin_favorite_count, admin_favorite_count);
        assert_eq!(new_last_edit_time, last_edit_time);
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn rate() -> ApiResult<()> {
        const POST_ID: i64 = 3;
        let get_post_info = |conn: &mut PgConnection| -> QueryResult<(i64, DateTime)> {
            post::table
                .inner_join(post_statistics::table)
                .select((post_statistics::score, post::last_edit_time))
                .filter(post_statistics::post_id.eq(POST_ID))
                .first(conn)
        };

        let mut conn = get_connection()?;
        let (score, last_edit_time) = get_post_info(&mut conn)?;

        verify_query(&format!("PUT /post/{POST_ID}/score/?{FIELDS}"), "post/like.json").await?;

        let (new_score, new_last_edit_time) = get_post_info(&mut conn)?;
        assert_eq!(new_score, score + 1);
        assert_eq!(new_last_edit_time, last_edit_time);

        verify_query(&format!("PUT /post/{POST_ID}/score/?{FIELDS}"), "post/dislike.json").await?;

        let (new_score, new_last_edit_time) = get_post_info(&mut conn)?;
        assert_eq!(new_score, score - 1);
        assert_eq!(new_last_edit_time, last_edit_time);

        verify_query(&format!("PUT /post/{POST_ID}/score/?{FIELDS}"), "post/remove_score.json").await?;

        let (new_score, new_last_edit_time) = get_post_info(&mut conn)?;
        assert_eq!(new_score, score);
        assert_eq!(new_last_edit_time, last_edit_time);
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn update() -> ApiResult<()> {
        const POST_ID: i64 = 5;
        let get_post_info = |conn: &mut PgConnection| -> QueryResult<(Post, i64, i64)> {
            post::table
                .inner_join(post_statistics::table)
                .select((Post::as_select(), post_statistics::tag_count, post_statistics::relation_count))
                .filter(post::id.eq(POST_ID))
                .first(conn)
        };

        let mut conn = get_connection()?;
        let (post, tag_count, relation_count) = get_post_info(&mut conn)?;

        verify_query(&format!("PUT /post/{POST_ID}/?{FIELDS}"), "post/update.json").await?;

        let (new_post, new_tag_count, new_relation_count) = get_post_info(&mut conn)?;
        assert_eq!(new_post.user_id, post.user_id);
        assert_eq!(new_post.file_size, post.file_size);
        assert_eq!(new_post.width, post.width);
        assert_eq!(new_post.height, post.height);
        assert_ne!(new_post.safety, post.safety);
        assert_eq!(new_post.type_, post.type_);
        assert_eq!(new_post.mime_type, post.mime_type);
        assert_eq!(new_post.checksum, post.checksum);
        assert_eq!(new_post.checksum_md5, post.checksum_md5);
        assert_ne!(new_post.flags, post.flags);
        assert_ne!(new_post.source, post.source);
        assert_eq!(new_post.description, post.description);
        assert_eq!(new_post.creation_time, post.creation_time);
        assert!(new_post.last_edit_time > post.last_edit_time);
        assert_ne!(new_tag_count, tag_count);
        assert_ne!(new_relation_count, relation_count);

        verify_query(&format!("PUT /post/{POST_ID}/?{FIELDS}"), "post/update_restore.json").await?;

        let new_tag_id: i64 = tag::table
            .select(tag::id)
            .inner_join(tag_name::table)
            .filter(tag_name::name.eq("new_tag"))
            .first(&mut conn)?;
        diesel::delete(tag::table.find(new_tag_id)).execute(&mut conn)?;

        let (new_post, new_tag_count, new_relation_count) = get_post_info(&mut conn)?;
        assert_eq!(new_post.user_id, post.user_id);
        assert_eq!(new_post.file_size, post.file_size);
        assert_eq!(new_post.width, post.width);
        assert_eq!(new_post.height, post.height);
        assert_eq!(new_post.safety, post.safety);
        assert_eq!(new_post.type_, post.type_);
        assert_eq!(new_post.mime_type, post.mime_type);
        assert_eq!(new_post.checksum, post.checksum);
        assert_eq!(new_post.checksum_md5, post.checksum_md5);
        assert_eq!(new_post.flags, post.flags);
        assert_eq!(new_post.source, post.source);
        assert_eq!(new_post.description, post.description);
        assert_eq!(new_post.creation_time, post.creation_time);
        assert!(new_post.last_edit_time > post.last_edit_time);
        assert_eq!(new_tag_count, tag_count);
        assert_eq!(new_relation_count, relation_count);
        Ok(())
    }
}
