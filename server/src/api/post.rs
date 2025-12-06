use crate::api::error::{ApiError, ApiResult};
use crate::api::extract::{Json, JsonOrMultipart, Path, Query};
use crate::api::{DeleteBody, MergeBody, PageParams, PagedResponse, RatingBody, ResourceParams};
use crate::app::AppState;
use crate::auth::Client;
use crate::content::hash::PostHash;
use crate::content::signature::SignatureCache;
use crate::content::thumbnail::{ThumbnailCategory, ThumbnailType};
use crate::content::upload::{MAX_UPLOAD_SIZE, PartName};
use crate::content::{Content, FileContents, signature, upload};
use crate::db::ConnectionPool;
use crate::filesystem::Directory;
use crate::model::enums::{PostFlag, PostFlags, PostSafety, PostType, ResourceType, Score};
use crate::model::post::{NewPost, NewPostFeature, NewPostSignature, Post, PostFavorite, PostScore, PostSignature};
use crate::resource::post::{Note, PostInfo};
use crate::schema::{post, post_favorite, post_feature, post_score, post_signature, post_statistics};
use crate::search::Builder;
use crate::search::post::QueryBuilder;
use crate::snapshot::post::SnapshotData;
use crate::string::{LargeString, SmallString};
use crate::time::DateTime;
use crate::{api, db, filesystem, resource, snapshot, update};
use axum::extract::{DefaultBodyLimit, Extension, State};
use axum::{Router, routing};
use diesel::dsl::exists;
use diesel::{
    Connection, ExpressionMethods, Insertable, OptionalExtension, QueryDsl, RunQueryDsl, SaveChangesDsl,
    SelectableHelper,
};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use tokio::sync::Mutex as AsyncMutex;
use tracing::info;
use url::Url;

pub fn routes() -> Router<AppState> {
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
async fn tagging_update<T, F>(connection_pool: &ConnectionPool, tags_updated: bool, update: F) -> ApiResult<T>
where
    F: FnOnce(&mut db::Connection) -> ApiResult<T>,
{
    let _lock;
    if tags_updated {
        _lock = POST_TAG_MUTEX.lock().await;
    }

    // Get a fresh connection here so that connection isn't being held while waiting on lock
    connection_pool.get()?.transaction(update)
}

/// See [listing-posts](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#listing-posts)
async fn list(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Query(params): Query<PageParams>,
) -> ApiResult<Json<PagedResponse<PostInfo>>> {
    api::verify_privilege(client, state.config.privileges().post_list)?;

    let offset = params.offset.unwrap_or(0);
    let limit = std::cmp::min(params.limit.get(), MAX_POSTS_PER_PAGE);
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    state.get_connection()?.transaction(|conn| {
        let mut query_builder = QueryBuilder::new(client, params.criteria())?;
        query_builder.set_offset_and_limit(offset, limit);

        let (total, selected_posts) = query_builder.list(conn)?;
        Ok(Json(PagedResponse {
            query: params.into_query(),
            offset,
            limit,
            total,
            results: PostInfo::new_batch_from_ids(conn, &state.config, client, &selected_posts, &fields)?,
        }))
    })
}

/// See [getting-post](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#getting-post)
async fn get(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(post_id): Path<i64>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<PostInfo>> {
    api::verify_privilege(client, state.config.privileges().post_view)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    state.get_connection()?.transaction(|conn| {
        let post_exists: bool = diesel::select(exists(post::table.find(post_id))).get_result(conn)?;
        if !post_exists {
            return Err(ApiError::NotFound(ResourceType::Post));
        }
        PostInfo::new_from_id(conn, &state.config, client, post_id, &fields)
            .map(Json)
            .map_err(ApiError::from)
    })
}

#[derive(Serialize)]
struct PostNeighbors {
    prev: Option<PostInfo>,
    next: Option<PostInfo>,
}

/// See [getting-around-post](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#getting-around-post)
async fn get_neighbors(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(post_id): Path<i64>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<PostNeighbors>> {
    api::verify_privilege(client, state.config.privileges().post_list)?;

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
    state.get_connection()?.transaction(|conn| {
        const INITIAL_LIMIT: i64 = 100;
        const LIMIT_GROWTH: i64 = 8;

        // Handle special cases first
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
            let post_infos = PostInfo::new_batch(conn, &state.config, client, posts, &fields)?;
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

        let post_ids: Vec<_> = prev_post_id.into_iter().chain(next_post_id).collect();
        let post_infos = PostInfo::new_batch_from_ids(conn, &state.config, client, &post_ids, &fields)?;
        Ok(create_post_neighbors(post_infos, prev_post_id.is_some()))
    })
}

/// See [getting-featured-post](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#getting-featured-post)
async fn get_featured(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<Option<PostInfo>>> {
    api::verify_privilege(client, state.config.privileges().post_view_featured)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    state.get_connection()?.transaction(|conn| {
        let featured_post_id: Option<i64> = post_feature::table
            .select(post_feature::post_id)
            .order_by(post_feature::time.desc())
            .first(conn)
            .optional()?;

        featured_post_id
            .map(|post_id| PostInfo::new_from_id(conn, &state.config, client, post_id, &fields))
            .transpose()
            .map(Json)
            .map_err(ApiError::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FeatureBody {
    id: i64,
}

/// See [featuring-post](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#featuring-post)
async fn feature(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<FeatureBody>,
) -> ApiResult<Json<PostInfo>> {
    api::verify_privilege(client, state.config.privileges().post_feature)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let user_id = client.id.ok_or(ApiError::NotLoggedIn)?;
    let new_post_feature = NewPostFeature {
        post_id: body.id,
        user_id,
        time: DateTime::now(),
    };

    let mut conn = state.get_connection()?;
    conn.transaction(|conn| {
        let previous_feature_id = post_feature::table
            .select(post_feature::post_id)
            .order_by(post_feature::time.desc())
            .first(conn)
            .optional()?;
        new_post_feature.insert_into(post_feature::table).execute(conn)?;
        snapshot::post::feature_snapshot(conn, client, previous_feature_id, body.id)
    })?;
    conn.transaction(|conn| PostInfo::new_from_id(conn, &state.config, client, body.id, &fields))
        .map(Json)
        .map_err(ApiError::from)
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

/// See [reverse-image-search](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#reverse-image-search)
async fn reverse_search(
    state: AppState,
    client: Client,
    params: ResourceParams,
    body: ReverseSearchBody,
) -> ApiResult<Json<ReverseSearchResponse>> {
    api::verify_privilege(client, state.config.privileges().post_reverse_search)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let content = Content::new(body.content, body.content_token, body.content_url)
        .ok_or(ApiError::MissingContent(ResourceType::Post))?;
    let content_properties = content.compute_properties(&state).await?;
    state
        .get_connection()?
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
                        .map(|post_id| PostInfo::new(conn, &state.config, client, post_id, &fields))
                        .transpose()?,
                    similar_posts: Vec::new(),
                });
            }

            // Search for similar images candidates
            let similar_signature_candidates = PostSignature::find_similar_candidates(
                conn,
                &signature::generate_indexes(&content_properties.signature),
            )?;
            info!("Found {} similar signatures", similar_signature_candidates.len());

            // Filter candidates based on similarity score
            let content_signature_cache = SignatureCache::new(&content_properties.signature);
            let mut similar_signatures: Vec<_> = similar_signature_candidates
                .into_iter()
                .filter_map(|post_signature| {
                    let distance = signature::distance(&content_signature_cache, &post_signature.signature);
                    let distance_threshold = 1.0 - state.config.post_similarity_threshold;
                    (distance < distance_threshold).then_some((post_signature.post_id, distance))
                })
                .collect();
            if similar_signatures.is_empty() {
                return Ok(ReverseSearchResponse {
                    exact_post: None,
                    similar_posts: Vec::new(),
                });
            }

            similar_signatures.sort_unstable_by(|(_, dist_a), (_, dist_b)| dist_a.total_cmp(dist_b));

            let (post_ids, distances): (Vec<_>, Vec<_>) = similar_signatures.into_iter().unzip();
            Ok(ReverseSearchResponse {
                exact_post: None,
                similar_posts: PostInfo::new_batch_from_ids(conn, &state.config, client, &post_ids, &fields)?
                    .into_iter()
                    .zip(distances)
                    .map(|(post, distance)| SimilarPost { distance, post })
                    .collect(),
            })
        })
        .map(Json)
}

/// Performs reverse image search using either a JSON body or a multipart form.
async fn reverse_search_handler(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
    body: JsonOrMultipart<ReverseSearchBody>,
) -> ApiResult<Json<ReverseSearchResponse>> {
    api::verify_privilege(client, state.config.privileges().post_reverse_search)?;

    match body {
        JsonOrMultipart::Json(payload) => reverse_search(state, client, params, payload).await,
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
                return Err(ApiError::MissingFormData);
            };
            reverse_search(state, client, params, reverse_search_body).await
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

/// See [creating-post](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#creating-post)
async fn create(
    state: AppState,
    client: Client,
    params: ResourceParams,
    body: CreateBody,
) -> ApiResult<Json<PostInfo>> {
    let required_rank = if body.anonymous.unwrap_or(false) {
        state.config.privileges().post_create_anonymous
    } else {
        state.config.privileges().post_create_identified
    };
    api::verify_privilege(client, required_rank)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let content = Content::new(body.content, body.content_token, body.content_url)
        .ok_or(ApiError::MissingContent(ResourceType::Post))?;
    let content_properties = content.get_or_compute_properties(&state).await?;

    let custom_thumbnail = match Content::new(body.thumbnail, body.thumbnail_token, body.thumbnail_url) {
        Some(content) => Some(content.thumbnail(&state.config, ThumbnailType::Post).await?),
        None => None,
    };
    let flags = content_properties.flags | PostFlags::from_slice(&body.flags.unwrap_or_default());

    let new_post = NewPost {
        user_id: client.id,
        file_size: content_properties.file_size,
        width: content_properties.width,
        height: content_properties.height,
        safety: body.safety,
        type_: PostType::from(content_properties.mime_type),
        mime_type: content_properties.mime_type,
        checksum: content_properties.checksum,
        checksum_md5: content_properties.md5_checksum,
        flags,
        source: body.source.as_deref().unwrap_or(""),
        description: body.description.as_deref().unwrap_or(""),
    };

    let post_id = tagging_update(&state.connection_pool, body.tags.is_some(), |conn| {
        // We do this before post insertion so that the post sequence isn't incremented if it fails
        let (tag_ids, tags) =
            update::tag::get_or_create_tag_ids(conn, &state.config, client, body.tags.unwrap_or_default(), false)?;
        let relations = body.relations.unwrap_or_default();
        let notes = body.notes.unwrap_or_default();

        let post: Post = new_post.insert_into(post::table).get_result(conn)?;
        let post_hash = PostHash::new(&state.config, post.id);

        // Add tags, relations, and notes
        update::post::set_tags(conn, post.id, &tag_ids)?;
        update::post::set_relations(conn, post.id, &relations)?;
        update::post::set_notes(conn, post.id, &notes)?;

        NewPostSignature {
            post_id: post.id,
            signature: content_properties.signature.into(),
            words: signature::generate_indexes(&content_properties.signature).into(),
        }
        .insert_into(post_signature::table)
        .execute(conn)?;

        // Move content to permanent location
        let temp_path = state
            .config
            .path(Directory::TemporaryUploads)
            .join(&content_properties.token);
        filesystem::move_file(&temp_path, &post_hash.content_path(content_properties.mime_type))?;

        // Create thumbnails
        if let Some(thumbnail) = custom_thumbnail {
            api::verify_privilege(client, state.config.privileges().post_edit_thumbnail)?;
            update::post::thumbnail(conn, &post_hash, &thumbnail, ThumbnailCategory::Custom)?;
        }
        update::post::thumbnail(conn, &post_hash, &content_properties.thumbnail, ThumbnailCategory::Generated)?;

        let post_data = SnapshotData {
            safety: post.safety,
            checksum: hex::encode(&post.checksum),
            flags: post.flags,
            source: post.source,
            description: post.description,
            tags,
            relations,
            notes,
            featured: false,
        };
        snapshot::post::creation_snapshot(conn, client, post.id, post_data)?;
        Ok::<_, ApiError>(post.id)
    })
    .await?;
    state
        .get_connection()?
        .transaction(|conn| PostInfo::new_from_id(conn, &state.config, client, post_id, &fields))
        .map(Json)
        .map_err(ApiError::from)
}

/// Creates a post from either a JSON body or a multipart form.
async fn create_handler(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
    body: JsonOrMultipart<CreateBody>,
) -> ApiResult<Json<PostInfo>> {
    match body {
        JsonOrMultipart::Json(payload) => create(state, client, params, payload).await,
        JsonOrMultipart::Multipart(payload) => {
            let decoded_body = upload::extract(payload, [PartName::Content, PartName::Thumbnail]).await?;
            let metadata = decoded_body.metadata.ok_or(ApiError::MissingMetadata)?;
            let mut new_post: CreateBody = serde_json::from_slice(&metadata)?;
            let [content, thumbnail] = decoded_body.files;

            new_post.content = content;
            new_post.thumbnail = thumbnail;
            create(state, client, params, new_post).await
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

/// See [merging-posts](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#merging-posts)
async fn merge(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<PostMergeBody>,
) -> ApiResult<Json<PostInfo>> {
    api::verify_privilege(client, state.config.privileges().post_merge)?;

    let absorbed_id = body.post_info.remove;
    let merge_to_id = body.post_info.merge_to;
    if absorbed_id == merge_to_id {
        return Err(ApiError::SelfMerge(ResourceType::Post));
    }

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    tagging_update(&state.connection_pool, true, |conn| {
        let absorbed_post: Post = post::table.find(absorbed_id).first(conn)?;
        let merge_to_post: Post = post::table.find(merge_to_id).first(conn)?;
        api::verify_version(absorbed_post.last_edit_time, body.post_info.remove_version)?;
        api::verify_version(merge_to_post.last_edit_time, body.post_info.merge_to_version)?;

        update::post::merge(conn, &state.config, &absorbed_post, &merge_to_post, body.replace_content)?;
        snapshot::post::merge_snapshot(conn, client, absorbed_id, merge_to_id)?;
        Ok(())
    })
    .await?;
    state
        .get_connection()?
        .transaction(|conn| PostInfo::new_from_id(conn, &state.config, client, body.post_info.merge_to, &fields))
        .map(Json)
        .map_err(ApiError::from)
}

/// See [adding-post-to-favorites](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#adding-post-to-favorites)
async fn favorite(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(post_id): Path<i64>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<PostInfo>> {
    api::verify_privilege(client, state.config.privileges().post_favorite)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let user_id = client.id.ok_or(ApiError::NotLoggedIn)?;
    let new_post_favorite = PostFavorite {
        post_id,
        user_id,
        time: DateTime::now(),
    };

    let mut conn = state.get_connection()?;
    conn.transaction(|conn| {
        diesel::delete(post_favorite::table.find((post_id, user_id))).execute(conn)?;
        new_post_favorite
            .insert_into(post_favorite::table)
            .execute(conn)
            .map_err(ApiError::from)
    })?;
    conn.transaction(|conn| PostInfo::new_from_id(conn, &state.config, client, post_id, &fields))
        .map(Json)
        .map_err(ApiError::from)
}

/// See [rating-post](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#getting-post)
async fn rate(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(post_id): Path<i64>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<RatingBody>,
) -> ApiResult<Json<PostInfo>> {
    api::verify_privilege(client, state.config.privileges().post_score)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let user_id = client.id.ok_or(ApiError::NotLoggedIn)?;

    let mut conn = state.get_connection()?;
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
        Ok::<_, ApiError>(())
    })?;
    conn.transaction(|conn| PostInfo::new_from_id(conn, &state.config, client, post_id, &fields))
        .map(Json)
        .map_err(ApiError::from)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct UpdateBody {
    version: DateTime,
    safety: Option<PostSafety>,
    source: Option<LargeString>,
    description: Option<LargeString>,
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

/// See [updating-post](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#updating-post)
async fn update(
    state: AppState,
    client: Client,
    post_id: i64,
    params: ResourceParams,
    body: UpdateBody,
) -> ApiResult<Json<PostInfo>> {
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let post_hash = PostHash::new(&state.config, post_id);

    let new_content = match Content::new(body.content, body.content_token, body.content_url) {
        Some(content) => Some(content.get_or_compute_properties(&state).await?),
        None => None,
    };
    let custom_thumbnail = match Content::new(body.thumbnail, body.thumbnail_token, body.thumbnail_url) {
        Some(content) => Some(content.thumbnail(&state.config, ThumbnailType::Post).await?),
        None => None,
    };

    tagging_update(&state.connection_pool, body.tags.is_some(), |conn| {
        let old_post: Post = post::table.find(post_id).first(conn)?;
        let old_mime_type = old_post.mime_type;
        api::verify_version(old_post.last_edit_time, body.version)?;

        let mut new_post = old_post.clone();
        let old_snapshot_data = SnapshotData::retrieve(conn, old_post)?;
        let mut new_snapshot_data = old_snapshot_data.clone();

        if let Some(safety) = body.safety {
            api::verify_privilege(client, state.config.privileges().post_edit_safety)?;

            new_post.safety = safety;
            new_snapshot_data.safety = safety;
        }
        if let Some(flags) = body.flags {
            api::verify_privilege(client, state.config.privileges().post_edit_flag)?;

            let updated_flags = PostFlags::from_slice(&flags);
            new_post.flags = updated_flags;
            new_snapshot_data.flags = updated_flags;
        }
        if let Some(source) = body.source {
            api::verify_privilege(client, state.config.privileges().post_edit_source)?;

            new_post.source = source.clone();
            new_snapshot_data.source = source;
        }
        if let Some(description) = body.description {
            api::verify_privilege(client, state.config.privileges().post_edit_description)?;

            new_post.description = description.clone();
            new_snapshot_data.description = description;
        }
        if let Some(relations) = body.relations {
            api::verify_privilege(client, state.config.privileges().post_edit_relation)?;

            update::post::set_relations(conn, post_id, &relations)?;
            new_snapshot_data.relations = relations;
        }
        if let Some(tags) = body.tags {
            api::verify_privilege(client, state.config.privileges().post_edit_tag)?;

            let (updated_tag_ids, tags) = update::tag::get_or_create_tag_ids(conn, &state.config, client, tags, false)?;
            update::post::set_tags(conn, post_id, &updated_tag_ids)?;
            new_snapshot_data.tags = tags;
        }
        if let Some(notes) = body.notes {
            api::verify_privilege(client, state.config.privileges().post_edit_note)?;

            update::post::set_notes(conn, post_id, &notes)?;
            new_snapshot_data.notes = notes;
        }
        if let Some(content_properties) = new_content {
            api::verify_privilege(client, state.config.privileges().post_edit_content)?;

            new_snapshot_data.checksum = hex::encode(&content_properties.checksum);

            // Update content metadata
            new_post.file_size = content_properties.file_size;
            new_post.width = content_properties.width;
            new_post.height = content_properties.height;
            new_post.type_ = PostType::from(content_properties.mime_type);
            new_post.mime_type = content_properties.mime_type;
            new_post.checksum = content_properties.checksum;
            new_post.checksum_md5 = content_properties.md5_checksum;
            new_post.flags |= content_properties.flags;

            // Update post signature
            let new_post_signature = NewPostSignature {
                post_id,
                signature: content_properties.signature.into(),
                words: signature::generate_indexes(&content_properties.signature).into(),
            };
            diesel::delete(post_signature::table.find(post_id)).execute(conn)?;
            new_post_signature.insert_into(post_signature::table).execute(conn)?;

            // Replace content
            let temp_path = state
                .config
                .path(Directory::TemporaryUploads)
                .join(&content_properties.token);
            filesystem::delete_content(&post_hash, old_mime_type)?;
            filesystem::move_file(&temp_path, &post_hash.content_path(content_properties.mime_type))?;

            // Replace generated thumbnail
            update::post::thumbnail(conn, &post_hash, &content_properties.thumbnail, ThumbnailCategory::Generated)?;
        }
        if let Some(thumbnail) = custom_thumbnail {
            api::verify_privilege(client, state.config.privileges().post_edit_thumbnail)?;
            update::post::thumbnail(conn, &post_hash, &thumbnail, ThumbnailCategory::Custom)?;
        }

        new_post.last_edit_time = DateTime::now();
        let _: Post = new_post.save_changes(conn)?;
        snapshot::post::modification_snapshot(conn, client, post_id, old_snapshot_data, new_snapshot_data)?;
        Ok(())
    })
    .await?;
    state
        .get_connection()?
        .transaction(|conn| PostInfo::new_from_id(conn, &state.config, client, post_id, &fields))
        .map(Json)
        .map_err(ApiError::from)
}

/// Updates post from either a JSON body or a multipart form.
async fn update_handler(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(post_id): Path<i64>,
    Query(params): Query<ResourceParams>,
    body: JsonOrMultipart<UpdateBody>,
) -> ApiResult<Json<PostInfo>> {
    match body {
        JsonOrMultipart::Json(payload) => update(state, client, post_id, params, payload).await,
        JsonOrMultipart::Multipart(payload) => {
            let decoded_body = upload::extract(payload, [PartName::Content, PartName::Thumbnail]).await?;
            let metadata = decoded_body.metadata.ok_or(ApiError::MissingMetadata)?;
            let mut post_update: UpdateBody = serde_json::from_slice(&metadata)?;
            let [content, thumbnail] = decoded_body.files;

            post_update.content = content;
            post_update.thumbnail = thumbnail;
            update(state, client, post_id, params, post_update).await
        }
    }
}

/// See [deleting-post](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#deleting-post)
async fn delete(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(post_id): Path<i64>,
    Json(client_version): Json<DeleteBody>,
) -> ApiResult<Json<()>> {
    // Post relation cascade deletion can cause deadlocks when deleting related posts in quick
    // succession, so we lock an aysnchronous mutex when deleting if the post has any relations.
    static ANTI_DEADLOCK_MUTEX: LazyLock<AsyncMutex<()>> = LazyLock::new(|| AsyncMutex::new(()));

    api::verify_privilege(client, state.config.privileges().post_delete)?;

    let relation_count: i64 = post_statistics::table
        .find(post_id)
        .select(post_statistics::relation_count)
        .first(&mut state.get_connection()?)?;

    let _lock;
    if relation_count > 0 {
        _lock = ANTI_DEADLOCK_MUTEX.lock().await;
    }

    let mut conn = state.get_connection()?;
    let mime_type = conn.transaction(|conn| {
        let post: Post = post::table.find(post_id).first(conn)?;
        api::verify_version(post.last_edit_time, *client_version)?;

        let mime_type = post.mime_type;
        let post_data = SnapshotData::retrieve(conn, post)?;
        snapshot::post::deletion_snapshot(conn, client, post_id, post_data)?;

        diesel::delete(post::table.find(post_id)).execute(conn)?;
        Ok::<_, ApiError>(mime_type)
    })?;
    if state.config.delete_source_files {
        filesystem::delete_post(&PostHash::new(&state.config, post_id), mime_type)?;
    }
    Ok(Json(()))
}

/// See [removing-post-from-favorites](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#removing-post-from-favorites)
async fn unfavorite(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(post_id): Path<i64>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<PostInfo>> {
    api::verify_privilege(client, state.config.privileges().post_favorite)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let user_id = client.id.ok_or(ApiError::NotLoggedIn)?;

    let mut conn = state.get_connection()?;
    diesel::delete(post_favorite::table.find((post_id, user_id))).execute(&mut conn)?;
    conn.transaction(|conn| PostInfo::new_from_id(conn, &state.config, client, post_id, &fields))
        .map(Json)
        .map_err(ApiError::from)
}

#[cfg(test)]
mod test {
    use crate::api::error::ApiResult;
    use crate::model::post::Post;
    use crate::schema::{post, post_feature, post_statistics, tag, tag_name, user, user_statistics};
    use crate::search::post::Token;
    use crate::test::*;
    use crate::time::DateTime;
    use diesel::dsl::exists;
    use diesel::{ExpressionMethods, PgConnection, QueryDsl, QueryResult, RunQueryDsl, SelectableHelper};
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
        verify_query(&format!("{QUERY}={SORT}{FIELDS}"), "post/list").await?;

        // Test sorts
        for token in Token::iter() {
            match token {
                Token::Id | Token::ContentChecksum | Token::NoteText | Token::Special => continue,
                _ => (),
            }
            let token_str: &'static str = token.into();
            let query = format!("{QUERY}=sort:{token_str} {SORT}&fields=id");
            let path = format!("post/list_{token_str}_sorted");
            verify_query(&query, &path).await?;
        }

        // Test filters
        verify_query(&format!("{QUERY}=-plant,sky,tagme {SORT}&fields=id"), "post/list_tag_filtered").await?;
        verify_query(&format!("{QUERY}=-pool:2 {SORT}&fields=id"), "post/list_pool_filtered").await?;
        verify_query(&format!("{QUERY}=fav:*user* {SORT}&fields=id"), "post/list_fav_filtered").await?;
        verify_query(&format!("{QUERY}=-comment:*user* {SORT}&fields=id"), "post/list_comment_filtered").await?;
        verify_query(&format!("{QUERY}=note-text:*fav* {SORT}&fields=id"), "post/list_note-text_filtered").await?;
        verify_query(&format!("{QUERY}=special:liked {SORT}&fields=id"), "post/list_liked_filtered").await?;
        verify_query(&format!("{QUERY}=special:disliked {SORT}&fields=id"), "post/list_disliked_filtered").await?;
        verify_query(&format!("{QUERY}=special:fav {SORT}&fields=id"), "post/list_special-fav_filtered").await?;
        verify_query(&format!("{QUERY}=special:tumbleweed {SORT}&fields=id"), "post/list_tumbleweed_filtered").await
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

        verify_query(&format!("GET /post/{POST_ID}/?{FIELDS}"), "post/get").await?;

        let new_last_edit_time = get_last_edit_time(&mut conn)?;
        assert_eq!(new_last_edit_time, last_edit_time);
        Ok(())
    }

    #[tokio::test]
    #[parallel]
    async fn get_neighbors() -> ApiResult<()> {
        const QUERY: &str = "around/?query=-sort:id";
        verify_query(&format!("GET /post/1/{QUERY}{FIELDS}"), "post/get_1_neighbors").await?;
        verify_query(&format!("GET /post/4/{QUERY}{FIELDS}"), "post/get_4_neighbors").await?;
        verify_query(&format!("GET /post/5/{QUERY}{FIELDS}"), "post/get_5_neighbors").await
    }

    #[tokio::test]
    #[parallel]
    async fn get_featured() -> ApiResult<()> {
        verify_query(&format!("GET /featured-post/?{FIELDS}"), "post/get_featured").await
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

        verify_query(&format!("POST /featured-post/?{FIELDS}"), "post/feature").await?;

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

        verify_query(&format!("POST /post-merge/?{FIELDS}"), "post/merge").await?;

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
        reset_database();
        Ok(())
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

        verify_query(&format!("POST /post/{POST_ID}/favorite/?{FIELDS}"), "post/favorite").await?;

        let (new_favorite_count, new_admin_favorite_count, new_last_edit_time) = get_post_info(&mut conn)?;
        assert_eq!(new_favorite_count, favorite_count + 1);
        assert_eq!(new_admin_favorite_count, admin_favorite_count + 1);
        assert_eq!(new_last_edit_time, last_edit_time);

        verify_query(&format!("DELETE /post/{POST_ID}/favorite/?{FIELDS}"), "post/unfavorite").await?;

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

        verify_query(&format!("PUT /post/{POST_ID}/score/?{FIELDS}"), "post/like").await?;

        let (new_score, new_last_edit_time) = get_post_info(&mut conn)?;
        assert_eq!(new_score, score + 1);
        assert_eq!(new_last_edit_time, last_edit_time);

        verify_query(&format!("PUT /post/{POST_ID}/score/?{FIELDS}"), "post/dislike").await?;

        let (new_score, new_last_edit_time) = get_post_info(&mut conn)?;
        assert_eq!(new_score, score - 1);
        assert_eq!(new_last_edit_time, last_edit_time);

        verify_query(&format!("PUT /post/{POST_ID}/score/?{FIELDS}"), "post/remove_score").await?;

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

        verify_query(&format!("PUT /post/{POST_ID}/?{FIELDS}"), "post/edit").await?;

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
        assert_ne!(new_post.description, post.description);
        assert_eq!(new_post.creation_time, post.creation_time);
        assert!(new_post.last_edit_time > post.last_edit_time);
        assert_ne!(new_tag_count, tag_count);
        assert_ne!(new_relation_count, relation_count);

        verify_query(&format!("PUT /post/{POST_ID}/?{FIELDS}"), "post/edit_restore").await?;

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
