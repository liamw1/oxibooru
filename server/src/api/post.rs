use crate::api::doc::POST_TAG;
use crate::api::error::{self, ApiError, ApiResult};
use crate::app::{AppState, Context};
use crate::config::Action;
use crate::content::hash::PostHash;
use crate::content::signature::SignatureCache;
use crate::content::thumbnail::{ThumbnailCategory, ThumbnailType};
use crate::content::upload::{MAX_UPLOAD_SIZE, PartName, UploadToken};
use crate::content::{Content, signature, upload};
use crate::db::AsyncConnectionPool;
use crate::extract::{
    Ctx, DeleteBody, Json, JsonOrMultipart, MergeBody, PageParams, PagedResponse, Path, Query, RatingBody,
    ResourceParams,
};
use crate::model::enums::{PostFlag, PostFlags, PostSafety, ResourceProperty, ResourceType, Score};
use crate::model::post::{
    NewPost, NewPostFavorite, NewPostFeature, NewPostScore, NewPostSignature, Post, PostSignature,
};
use crate::resource::post::{Field, Note, PostInfo};
use crate::schema::{post, post_favorite, post_feature, post_score, post_signature, post_statistics};
use crate::search::Builder;
use crate::search::post::QueryBuilder;
use crate::snapshot::post::SnapshotData;
use crate::string::{LargeString, SmallString};
use crate::time::DateTime;
use crate::update::tag::FetchMode;
use crate::{api, db, filesystem, snapshot, update};
use axum::extract::DefaultBodyLimit;
use diesel::dsl::{exists, not};
use diesel::{
    ExpressionMethods, Insertable, OptionalExtension, PgConnection, QueryDsl, RunQueryDsl, SaveChangesDsl,
    SelectableHelper,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock};
use tokio::sync::Mutex as AsyncMutex;
use tracing::info;
use url::Url;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

pub fn routes() -> OpenApiRouter<AppState> {
    let upload_capable_routes = OpenApiRouter::new()
        .routes(routes!(reverse_search))
        .routes(routes!(create))
        .routes(routes!(update))
        .route_layer(DefaultBodyLimit::max(MAX_UPLOAD_SIZE));
    OpenApiRouter::new()
        .routes(routes!(list))
        .routes(routes!(get, delete))
        .routes(routes!(get_neighbors))
        .routes(routes!(get_featured, feature))
        .routes(routes!(merge))
        .routes(routes!(favorite, unfavorite))
        .routes(routes!(rate))
        .merge(upload_capable_routes)
}

static POST_TAG_MUTEX: LazyLock<AsyncMutex<()>> = LazyLock::new(|| AsyncMutex::new(()));

#[allow(dead_code)]
#[derive(ToSchema)]
struct Multipart<T> {
    /// JSON metadata (same structure as JSON request body).
    metadata: T,
    /// Content file (image, video, etc.).
    #[schema(format = Binary)]
    content: Option<String>,
    /// Thumbnail file.
    #[schema(format = Binary)]
    thumbnail: Option<String>,
}

fn verify_visibility(conn: &mut PgConnection, ctx: &Context, post_id: i64) -> ApiResult<()> {
    let post_exists: bool = diesel::select(exists(post::table.find(post_id))).first(conn)?;
    if !post_exists {
        return Err(ApiError::NotFound(ResourceType::Post));
    }

    if let Some(hidden_posts) = ctx.preferences().hidden_posts(post_statistics::post_id) {
        let post_lookup = hidden_posts.filter(post_statistics::post_id.eq(post_id));
        let post_hidden: bool = diesel::select(exists(post_lookup)).first(conn)?;
        if post_hidden {
            return Err(ApiError::Hidden(ResourceType::Post));
        }
    }
    Ok(())
}

/// Runs an `update` that may add `tags` to a post as a transaction.
///
/// Tagging multiple posts simultaneously can cause issues if two updates share tags.
/// If two updates share a new tag, a uniqueness violation can occur.
/// If two updates share an existing tag, a deadlock can occur due to statistics updating.
/// Therefore, we lock an asynchronous mutex whenever updating post tags. This is a bit
/// more pessimistic than necessary, as parallel updates are safe if the sets of tags are
/// disjoint. However, allowing disjoint tagging introduces additional complexity so it
/// isn't being done as of now.
async fn tagging_update<T, F>(connection_pool: &AsyncConnectionPool, tags_updated: bool, update: F) -> ApiResult<T>
where
    T: Send + 'static,
    F: FnOnce(&mut db::Connection) -> ApiResult<T> + Send + 'static,
{
    let _lock;
    if tags_updated {
        _lock = POST_TAG_MUTEX.lock().await;
    }

    // Get a fresh connection here so that connection isn't being held while waiting on lock
    connection_pool.transaction(update).await
}

/// Searches for posts.
///
/// **Anonymous tokens**
///
/// Same as `tag` token.
///
/// **Named tokens**
///
/// | Key                                                          | Description                                                             |
/// | ------------------------------------------------------------ | ----------------------------------------------------------------------- |
/// | `id`                                                         | having given post number                                                |
/// | `tag`                                                        | having given tag (accepts wildcards)                                    |
/// | `tag-category`                                               | having tags from given tag category (accepts wildcards)                 |
/// | `score`                                                      | having given score                                                      |
/// | `uploader`, `upload`, `submit`                               | uploaded by given user (accepts wildcards)                              |
/// | `comment`                                                    | commented by given user (accepts wildcards)                             |
/// | `fav`                                                        | favorited by given user (accepts wildcards)                             |
/// | `pool`                                                       | belonging to the pool with the given name (accepts wildcards) or ID     |
/// | `pool-category`                                              | belonging to pools in the given pool category (accepts wildcards)       |
/// | `tag-count`                                                  | having given number of tags                                             |
/// | `comment-count`                                              | having given number of comments                                         |
/// | `fav-count`                                                  | favorited by given number of users                                      |
/// | `note-count`                                                 | having given number of annotations                                      |
/// | `note-text`                                                  | having given note text (accepts wildcards)                              |
/// | `relation-count`                                             | having given number of relations                                        |
/// | `feature-count`                                              | having been featured given number of times                              |
/// | `type`                                                       | type of posts (can be either `image`, `animation`, `flash`, or `video`) |
/// | `content-checksum`                                           | having given BLAKE3 checksum                                            |
/// | `flag`                                                       | having given flag (can be either `loop` or `sound`)                     |
/// | `source`                                                     | having given source                                                     |
/// | `file-size`                                                  | having given file size (in bytes)                                       |
/// | `image-width`, `width`                                       | having given image width (where applicable)                             |
/// | `image-height`, `height`                                     | having given image height (where applicable)                            |
/// | `image-area`, `area`                                         | having given number of pixels (image width * image height)              |
/// | `image-aspect-ratio`, `image-ar`, `ar`, `aspect-ratio`       | having given aspect ratio (image width / image height)                  |
/// | `creation-date`, `creation-time`, `date`, `time`             | posted at given date                                                    |
/// | `last-edit-date`, `last-edit-time`, `edit-date`, `edit-time` | edited at given date                                                    |
/// | `comment-date`, `comment-time`                               | commented at given date                                                 |
/// | `fav-date`, `fav-time`                                       | last favorited at given date                                            |
/// | `feature-date`, `feature-time`                               | featured at given date                                                  |
/// | `safety`, `rating`                                           | having given safety (can be either `safe`, `sketchy`, or `unsafe`)      |
///
/// **Sort style tokens**
///
/// | Value                                                        | Description                                      |
/// | ------------------------------------------------------------ | ------------------------------------------------ |
/// | `random`                                                     | as random as it can get                          |
/// | `id`                                                         | highest to lowest post number                    |
/// | `score`                                                      | highest scored                                   |
/// | `uploader`, `upload`, `submit`                               | uploader name alphabetically                     |
/// | `pool-count`, `pool`                                         | in most pools                                    |
/// | `tag-count`, `tag`                                           | with most tags                                   |
/// | `comment-count`, `comment`                                   | most commented first                             |
/// | `fav-count`, `fav`                                           | loved by most                                    |
/// | `note-count`                                                 | with most annotations                            |
/// | `relation-count`                                             | with most relations                              |
/// | `feature-count`                                              | most often featured                              |
/// | `type`                                                       | grouped by content type                          |
/// | `flag`                                                       | grouped by flags                                 |
/// | `source`                                                     | sorted by source                                 |
/// | `file-size`                                                  | largest files first                              |
/// | `image-width`, `width`                                       | widest images first                              |
/// | `image-height`, `height`                                     | tallest images first                             |
/// | `image-area`, `area`                                         | largest images first                             |
/// | `image-aspect-ratio`, `image-ar`, `ar`, `aspect-ratio`       | highest aspect ratio first                       |
/// | `creation-date`, `creation-time`, `date`, `time`             | newest to oldest (pretty much same as id)        |
/// | `last-edit-date`, `last-edit-time`, `edit-date`, `edit-time` | like creation-date, only looks at last edit time |
/// | `comment-date`, `comment-time`                               | recently commented by anyone                     |
/// | `fav-date`, `fav-time`                                       | recently added to favorites by anyone            |
/// | `feature-date`, `feature-time`                               | recently featured                                |
/// | `safety`, `rating`                                           | most unsafe first                                |
///
/// **Special tokens**
///
/// | Value        | Description                                                   |
/// | ------------ | ------------------------------------------------------------- |
/// | `liked`      | posts liked by currently logged in user                       |
/// | `disliked`   | posts disliked by currently logged in user                    |
/// | `fav`        | posts added to favorites by currently logged in user          |
/// | `tumbleweed` | posts with score of 0, without comments and without favorites |
#[utoipa::path(
    get,
    path = "/posts",
    tag = POST_TAG,
    params(ResourceParams, PageParams),
    responses(
        (status = 200, body = PagedResponse<PostInfo>),
        (status = 403, description = "Privileges are too low"),
    ),
)]
async fn list(
    Ctx(ctx, connection_pool): Ctx,
    Query(resource): Query<ResourceParams<Field>>,
    Query(page): Query<PageParams>,
) -> ApiResult<Json<PagedResponse<PostInfo>>> {
    ctx.verify_privilege(Action::PostView)?;
    ctx.verify_privilege(Action::PostList)?;

    let offset = page.offset.unwrap_or(0);
    let limit = page.limit();
    connection_pool
        .transaction(move |conn| {
            let mut query_builder = QueryBuilder::new(&ctx, resource.criteria())?;
            query_builder.set_offset_and_limit(offset, limit);

            let (total, selected_posts) = query_builder.list(conn)?;
            Ok::<_, ApiError>(Json(PagedResponse {
                query: resource.query,
                offset,
                limit,
                total,
                results: PostInfo::new_batch_from_ids(conn, &ctx, &selected_posts, resource.fields)?,
            }))
        })
        .await
}

/// Retrieves information about an existing post.
#[utoipa::path(
    get,
    path = "/post/{id}",
    tag = POST_TAG,
    params(
        ("id" = i64, Path, description = "Post ID"),
        ResourceParams,
    ),
    responses(
        (status = 200, body = PostInfo),
        (status = 403, description = "Privileges are too low"),
        (status = 403, description = "Post is hidden"),
        (status = 404, description = "Post does not exist"),
    ),
)]
async fn get(
    Ctx(ctx, connection_pool): Ctx,
    Path(post_id): Path<i64>,
    Query(params): Query<ResourceParams<Field>>,
) -> ApiResult<Json<PostInfo>> {
    ctx.verify_privilege(Action::PostView)?;

    connection_pool
        .transaction(move |conn| {
            verify_visibility(conn, &ctx, post_id)?;
            PostInfo::new_from_id(conn, &ctx, post_id, params.fields)
                .map(Json)
                .map_err(ApiError::from)
        })
        .await
}

/// Response containing neighboring posts.
#[derive(Serialize, ToSchema)]
struct PostNeighbors {
    /// The previous post, or null if none.
    prev: Option<PostInfo>,
    /// The next post, or null if none.
    next: Option<PostInfo>,
}

/// Retrieves information about posts that are before or after an existing post.
#[utoipa::path(
    get,
    path = "/post/{id}/around",
    tag = POST_TAG,
    params(
        ("id" = i64, Path, description = "Post ID"),
        ResourceParams,
    ),
    responses(
        (status = 200, body = PostNeighbors),
        (status = 403, description = "Privileges are too low"),
        (status = 403, description = "Post is hidden"),
        (status = 404, description = "Post does not exist"),
    ),
)]
async fn get_neighbors(
    Ctx(ctx, connection_pool): Ctx,
    Path(post_id): Path<i64>,
    Query(params): Query<ResourceParams<Field>>,
) -> ApiResult<Json<PostNeighbors>> {
    ctx.verify_privilege(Action::PostView)?;

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

    connection_pool
        .transaction(move |conn| {
            const INITIAL_LIMIT: u64 = 1000;
            const LIMIT_GROWTH: u64 = 10;

            verify_visibility(conn, &ctx, post_id)?;

            let mut query_builder = QueryBuilder::new(&ctx, params.criteria())?;

            if !query_builder.criteria().has_sort() {
                // Optimized neighbor retrieval if no sort is specified
                let previous_post = query_builder
                    .build_filtered(conn)?
                    .select(Post::as_select())
                    .filter(post::id.gt(post_id))
                    .order(post::id.asc())
                    .first(conn)
                    .optional()?;
                let next_post = query_builder
                    .build_filtered(conn)?
                    .select(Post::as_select())
                    .filter(post::id.lt(post_id))
                    .order(post::id.desc())
                    .first(conn)
                    .optional()?;

                let has_previous_post = previous_post.is_some();
                let posts = previous_post.into_iter().chain(next_post).collect();
                let post_infos = PostInfo::new_batch(conn, &ctx, posts, params.fields)?;
                return Ok::<_, ApiError>(create_post_neighbors(post_infos, has_previous_post));
            }

            // Search for neighbors using exponentially increasing limit
            let mut offset = 0;
            let mut limit = INITIAL_LIMIT;
            let (prev_post_id, next_post_id) = loop {
                query_builder.set_offset_and_limit(offset, limit);
                let post_id_batch = query_builder.load(conn)?;

                let post_position = post_id_batch.iter().position(|&id| id == post_id);
                if post_id_batch.len() < usize::try_from(limit).unwrap_or(usize::MAX)
                    || limit == u64::MAX
                    || post_id_batch.len() == usize::MAX
                    || (post_position.is_some() && post_position != Some(post_id_batch.len().saturating_sub(1)))
                {
                    let prev_post_id = post_position
                        .and_then(|index| index.checked_sub(1))
                        .and_then(|prev_index| post_id_batch.get(prev_index))
                        .copied();
                    let next_post_id = post_position
                        .and_then(|index| index.checked_add(1))
                        .and_then(|next_index| post_id_batch.get(next_index))
                        .copied();
                    break (prev_post_id, next_post_id);
                }

                offset += limit - 2;
                limit = limit.saturating_mul(LIMIT_GROWTH);
            };

            let post_ids: Vec<_> = prev_post_id.into_iter().chain(next_post_id).collect();
            let post_infos = PostInfo::new_batch_from_ids(conn, &ctx, &post_ids, params.fields)?;
            Ok(create_post_neighbors(post_infos, prev_post_id.is_some()))
        })
        .await
}

/// Retrieves the post that is currently featured on the main page in web client.
///
/// If no post is featured, the response is null. Note that this method exists
/// mostly for compatibility with setting featured post - most of the time,
/// you'd want to use query global info which contains more information.
#[utoipa::path(
    get,
    path = "/featured-post",
    tag = POST_TAG,
    params(ResourceParams),
    responses(
        (status = 200, body = Option<PostInfo>),
        (status = 403, description = "Privileges are too low"),
    ),
)]
async fn get_featured(
    Ctx(ctx, connection_pool): Ctx,
    Query(params): Query<ResourceParams<Field>>,
) -> ApiResult<Json<Option<PostInfo>>> {
    ctx.verify_privilege(Action::PostViewFeatured)?;

    connection_pool
        .transaction(move |conn| {
            let mut featured = post_feature::table
                .select(post_feature::post_id)
                .order(post_feature::time.desc())
                .into_boxed();

            // Apply preferences to post features
            if let Some(hidden_posts) = ctx.preferences().hidden_posts(post_feature::post_id) {
                featured = featured.filter(not(exists(hidden_posts)));
            }

            let featured_post_id: Option<i64> = featured.first(conn).optional()?;
            featured_post_id
                .map(|post_id| PostInfo::new_from_id(conn, &ctx, post_id, params.fields))
                .transpose()
                .map(Json)
                .map_err(ApiError::from)
        })
        .await
}

/// Request body for featuring a post.
#[derive(Deserialize, ToSchema)]
struct FeatureBody {
    /// ID of the post to feature.
    id: i64,
}

/// Features a post on the main page in web client.
#[utoipa::path(
    post,
    path = "/featured-post",
    tag = POST_TAG,
    params(ResourceParams),
    request_body = FeatureBody,
    responses(
        (status = 200, body = PostInfo),
        (status = 403, description = "Privileges are too low"),
        (status = 409, description = "Trying to feature a post that is currently featured"),
    ),
)]
async fn feature(
    Ctx(ctx, connection_pool): Ctx,
    Query(params): Query<ResourceParams<Field>>,
    Json(body): Json<FeatureBody>,
) -> ApiResult<Json<PostInfo>> {
    ctx.verify_privilege(Action::PostView)?;
    ctx.verify_privilege(Action::PostFeature)?;

    let user_id = ctx.client.id.ok_or(ApiError::NotLoggedIn)?;
    let new_post_feature = NewPostFeature {
        post_id: body.id,
        user_id,
    };

    connection_pool
        .transaction(move |conn| {
            let previous_feature_id = post_feature::table
                .select(post_feature::post_id)
                .order(post_feature::time.desc())
                .first(conn)
                .optional()?;
            if previous_feature_id == Some(new_post_feature.post_id) {
                return Err(ApiError::AlreadyExists(ResourceProperty::PostFeature));
            }

            let insert_result = new_post_feature.insert_into(post_feature::table).execute(conn);
            error::map_foreign_key_violation(insert_result, ResourceType::Post)?;
            snapshot::post::feature_snapshot(conn, ctx.client, previous_feature_id, body.id)?;
            Ok::<_, ApiError>(())
        })
        .await?;
    connection_pool
        .transaction(move |conn| PostInfo::new_from_id(conn, &ctx, body.id, params.fields))
        .await
        .map(Json)
}

async fn reverse_search_impl(
    ctx: Ctx,
    params: ResourceParams<Field>,
    body: ReverseSearchBody,
) -> ApiResult<Json<ReverseSearchResponse>> {
    let content =
        Content::new(body.content_token, body.content_url).ok_or(ApiError::MissingContent(ResourceType::Post))?;
    let content_properties = content.compute_properties(ctx.clone()).await?;

    let Ctx(ctx, connection_pool) = ctx;
    connection_pool
        .transaction(move |conn| {
            let _timer = crate::time::Timer::new("reverse search");

            // Check for exact match
            let exact_post: Option<Post> = post::table
                .filter(post::checksum.eq(content_properties.checksum))
                .first(conn)
                .optional()?;

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
                .filter(|post_signature| Some(post_signature.post_id) != exact_post.as_ref().map(|post| post.id))
                .filter_map(|post_signature| {
                    let distance = signature::distance(&content_signature_cache, &post_signature.signature);
                    let distance_threshold = 1.0 - ctx.config.post_similarity_threshold;
                    (distance < distance_threshold).then_some((post_signature.post_id, distance))
                })
                .collect();
            similar_signatures.sort_unstable_by(|(_, dist_a), (_, dist_b)| dist_a.total_cmp(dist_b));

            let (post_ids, distances): (Vec<_>, Vec<_>) = similar_signatures.into_iter().unzip();
            Ok::<_, ApiError>(ReverseSearchResponse {
                exact_post: exact_post
                    .map(|post| PostInfo::new(conn, &ctx, post, params.fields))
                    .transpose()?,
                similar_posts: PostInfo::new_batch_from_ids(conn, &ctx, &post_ids, params.fields)?
                    .into_iter()
                    .zip(distances)
                    .map(|(post, distance)| SimilarPost { distance, post })
                    .collect(),
            })
        })
        .await
        .map(Json)
}

/// Request body for reverse image search.
#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ReverseSearchBody {
    /// Token referencing previously uploaded content.
    #[schema(value_type = Option<String>)]
    content_token: Option<UploadToken>,
    /// URL to fetch image content from.
    content_url: Option<Url>,
}

/// A post with its visual similarity distance.
#[derive(Serialize, ToSchema)]
struct SimilarPost {
    /// Visual similarity distance. Lower is more similar.
    #[schema(minimum = 0.0, maximum = 1.0)]
    distance: f64,
    /// The similar post.
    post: PostInfo,
}

/// Response from reverse image search.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ReverseSearchResponse {
    /// Exact match if found (same checksum).
    exact_post: Option<PostInfo>,
    /// Posts that are visually similar to the input image.
    similar_posts: Vec<SimilarPost>,
}

/// Retrieves posts that look like the input image.
#[utoipa::path(
    post,
    path = "/posts/reverse-search",
    tag = POST_TAG,
    params(ResourceParams),
    request_body(
        content(
            (ReverseSearchBody = "application/json"),
            (Multipart<ReverseSearchBody> = "multipart/form-data"),
        )
    ),
    responses(
        (status = 200, body = ReverseSearchResponse),
        (status = 400, description = "Reverse search content is missing"),
        (status = 403, description = "Privileges are too low"),
    ),
)]
async fn reverse_search(
    ctx: Ctx,
    Query(params): Query<ResourceParams<Field>>,
    body: JsonOrMultipart<ReverseSearchBody>,
) -> ApiResult<Json<ReverseSearchResponse>> {
    ctx.verify_privilege(Action::PostView)?;
    ctx.verify_privilege(Action::PostReverseSearch)?;

    match body {
        JsonOrMultipart::Json(payload) => reverse_search_impl(ctx, params, payload).await,
        JsonOrMultipart::Multipart(payload) => {
            let decoded_body = upload::extract(&ctx.config, payload, [PartName::Content]).await?;
            let reverse_search_body = if let [Some(content_token)] = decoded_body.files {
                ReverseSearchBody {
                    content_token: Some(content_token),
                    content_url: None,
                }
            } else if let Some(metadata) = decoded_body.metadata {
                serde_json::from_slice(&metadata)?
            } else {
                return Err(ApiError::MissingFormData);
            };
            reverse_search_impl(ctx, params, reverse_search_body).await
        }
    }
}

async fn create_impl(ctx: Ctx, params: ResourceParams<Field>, body: PostCreateBody) -> ApiResult<Json<PostInfo>> {
    let action = if body.anonymous.unwrap_or(false) {
        Action::PostCreateAnonymous
    } else {
        Action::PostCreateIdentified
    };
    ctx.verify_privilege(action)?;

    let content =
        Content::new(body.content_token, body.content_url).ok_or(ApiError::MissingContent(ResourceType::Post))?;
    let content_properties = content.remove_or_compute_properties(ctx.clone()).await?;

    let custom_thumbnail = match Content::new(body.thumbnail_token, body.thumbnail_url) {
        Some(content) => Some(content.thumbnail(ctx.clone(), ThumbnailType::Post).await?),
        None => None,
    };
    let flags = content_properties.flags | PostFlags::from_slice(&body.flags.unwrap_or_default());

    let Ctx(ctx, connection_pool) = ctx;
    let post_id = tagging_update(&connection_pool, body.tags.is_some(), {
        let ctx = ctx.clone();
        move |conn| {
            // We do this before post insertion so that the post sequence isn't incremented if it fails
            let (tag_ids, tags) =
                update::tag::get_or_create_tags(conn, &ctx, body.tags.unwrap_or_default(), FetchMode::Deep)?;
            let relations = body.relations.unwrap_or_default();
            let notes = body.notes.unwrap_or_default();

            let post: Post = NewPost {
                user_id: ctx.client.id,
                file_size: content_properties.file_size,
                width: content_properties.width,
                height: content_properties.height,
                safety: body.safety,
                type_: content_properties.post_type,
                mime_type: content_properties.mime_type,
                checksum: content_properties.checksum,
                checksum_md5: content_properties.md5_checksum,
                flags,
                source: body.source.as_deref().unwrap_or(""),
                description: body.description.as_deref().unwrap_or(""),
            }
            .insert_into(post::table)
            .on_conflict(post::checksum)
            .do_nothing()
            .get_result(conn)
            .optional()?
            .ok_or(ApiError::AlreadyExists(ResourceProperty::PostContent))?;
            let post_hash = PostHash::new(&ctx.config, post.id, Some(post.custom_thumbnail_size));

            // Add tags, relations, and notes
            update::post::set_tags(conn, post.id, &tag_ids)?;
            update::post::add_relations(conn, post.id, &relations)?;
            update::post::set_notes(conn, post.id, &notes)?;

            NewPostSignature {
                post_id: post.id,
                signature: content_properties.signature.into(),
                words: signature::generate_indexes(&content_properties.signature).into(),
            }
            .insert_into(post_signature::table)
            .execute(conn)?;

            // Move content to permanent location
            let temp_path = content_properties.token.path(&ctx.config);
            filesystem::move_file(&temp_path, &post_hash.content_path(content_properties.mime_type))?;

            // Create thumbnails
            if let Some(thumbnail) = custom_thumbnail {
                ctx.verify_privilege(Action::PostEditThumbnail)?;
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
            snapshot::post::creation_snapshot(conn, ctx.client, post.id, post_data)?;
            Ok::<_, ApiError>(post.id)
        }
    })
    .await?;
    connection_pool
        .transaction(move |conn| PostInfo::new_from_id(conn, &ctx, post_id, params.fields))
        .await
        .map(Json)
}

/// Request body for creating a post.
#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct PostCreateBody {
    /// Post safety rating.
    safety: PostSafety,
    /// Token referencing previously uploaded content.
    #[schema(value_type = Option<String>)]
    content_token: Option<UploadToken>,
    /// URL to fetch content from.
    content_url: Option<Url>,
    /// Token referencing previously uploaded thumbnail.
    #[schema(value_type = Option<String>)]
    thumbnail_token: Option<UploadToken>,
    /// URL to fetch thumbnail from.
    thumbnail_url: Option<Url>,
    /// Source URL or description.
    source: Option<String>,
    /// Post description.
    description: Option<String>,
    /// IDs of related posts.
    relations: Option<Vec<i64>>,
    /// If true, the uploader name won't be recorded.
    anonymous: Option<bool>,
    /// Tags to apply. Non-existent tags will be created automatically.
    tags: Option<Vec<SmallString>>,
    /// Post annotations.
    notes: Option<Vec<Note>>,
    /// Post flags: `loop` or `sound`.
    flags: Option<Vec<PostFlag>>,
}

/// Creates a new post.
///
/// If specified tags do not exist yet, they will be automatically created.
/// Tags created automatically have no implications, no suggestions, one name
/// and their category is set to the first tag category found. Safety must be
/// any of `safe`, `sketchy` or `unsafe`. Relations must contain valid post IDs.
/// If `flags` is omitted, they will be defined by default (`loop` will be set
/// for all video posts, and `sound` will be auto-detected). Sending empty
/// `thumbnail` will cause the post to use default thumbnail. If `anonymous` is
/// set to truthy value, the uploader name won't be recorded (privilege
/// verification still applies; it's possible to disallow anonymous uploads
/// completely from config.) For details on how to pass `content` and
/// `thumbnail`, see [file uploads](#Upload).
#[utoipa::path(
    post,
    path = "/posts",
    tag = POST_TAG,
    params(ResourceParams),
    request_body(
        content(
            (PostCreateBody = "application/json"),
            (Multipart<PostCreateBody> = "multipart/form-data"),
        )
    ),
    responses(
        (status = 200, body = PostInfo),
        (status = 400, description = "Post content is missing"),
        (status = 403, description = "Privileges are too low"),
        (status = 404, description = "Relations refer to non-existing posts"),
        (status = 409, description = "Post content already exists"),
        (status = 422, description = "A Tag has an invalid name"),
        (status = 422, description = "Safety is invalid"),
        (status = 422, description = "Notes are invalid"),
        (status = 422, description = "Flags are invalid"),
    ),
)]
async fn create(
    ctx: Ctx,
    Query(params): Query<ResourceParams<Field>>,
    body: JsonOrMultipart<PostCreateBody>,
) -> ApiResult<Json<PostInfo>> {
    match body {
        JsonOrMultipart::Json(payload) => create_impl(ctx, params, payload).await,
        JsonOrMultipart::Multipart(payload) => {
            let decoded_body = upload::extract(&ctx.config, payload, [PartName::Content, PartName::Thumbnail]).await?;
            let metadata = decoded_body.metadata.ok_or(ApiError::MissingMetadata)?;
            let mut new_post: PostCreateBody = serde_json::from_slice(&metadata)?;
            let [content_token, thumbnail_token] = decoded_body.files;

            new_post.content_token = content_token;
            new_post.thumbnail_token = thumbnail_token;
            create_impl(ctx, params, new_post).await
        }
    }
}

/// Request body for merging posts.
#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct PostMergeBody {
    #[schema(inline)]
    #[serde(flatten)]
    post_info: MergeBody<i64>,
    /// If true, replace target post content with source post content.
    replace_content: bool,
}

/// Removes source post and merges all of its data to the target post.
///
/// Merges all tags, relations, scores, favorites and comments from the source
/// to the target post. If `replaceContent` is set to true, content of the
/// target post is replaced using the content of the source post; otherwise it
/// remains unchanged. Source post properties such as its safety, source,
/// whether to loop the video and other scalar values do not get transferred
/// and are discarded.
#[utoipa::path(
    post,
    path = "/post-merge",
    tag = POST_TAG,
    params(ResourceParams),
    request_body = PostMergeBody,
    responses(
        (status = 200, body = PostInfo),
        (status = 403, description = "Privileges are too low"),
        (status = 404, description = "Source or target post does not exist"),
        (status = 409, description = "Version of either post is outdated"),
        (status = 422, description = "Source post is the same as the target post"),
    ),
)]
async fn merge(
    Ctx(ctx, connection_pool): Ctx,
    Query(params): Query<ResourceParams<Field>>,
    Json(body): Json<PostMergeBody>,
) -> ApiResult<Json<PostInfo>> {
    ctx.verify_privilege(Action::PostView)?;
    ctx.verify_privilege(Action::PostMerge)?;

    let absorbed_id = body.post_info.remove;
    let merge_to_id = body.post_info.merge_to;
    if absorbed_id == merge_to_id {
        return Err(ApiError::SelfMerge(ResourceType::Post));
    }

    tagging_update(&connection_pool, true, {
        let config = Arc::clone(&ctx.config);
        move |conn| {
            let absorbed_post: Post = post::table
                .find(absorbed_id)
                .first(conn)
                .optional()?
                .ok_or(ApiError::NotFound(ResourceType::Post))?;
            let merge_to_post: Post = post::table
                .find(merge_to_id)
                .first(conn)
                .optional()?
                .ok_or(ApiError::NotFound(ResourceType::Post))?;
            api::verify_version(absorbed_post.last_edit_time, body.post_info.remove_version)?;
            api::verify_version(merge_to_post.last_edit_time, body.post_info.merge_to_version)?;

            update::post::merge(conn, &config, &absorbed_post, &merge_to_post, body.replace_content)?;
            snapshot::post::merge_snapshot(conn, ctx.client, absorbed_id, merge_to_id)?;
            Ok(())
        }
    })
    .await?;
    connection_pool
        .transaction(move |conn| PostInfo::new_from_id(conn, &ctx, body.post_info.merge_to, params.fields))
        .await
        .map(Json)
}

/// Marks the post as favorite for authenticated user.
#[utoipa::path(
    post,
    path = "/post/{id}/favorite",
    tag = POST_TAG,
    params(
        ("id" = i64, Path, description = "Post ID"),
        ResourceParams,
    ),
    responses(
        (status = 200, body = PostInfo),
        (status = 403, description = "Privileges are too low"),
        (status = 404, description = "Post does not exist"),
    ),
)]
async fn favorite(
    Ctx(ctx, connection_pool): Ctx,
    Path(post_id): Path<i64>,
    Query(params): Query<ResourceParams<Field>>,
) -> ApiResult<Json<PostInfo>> {
    ctx.verify_privilege(Action::PostView)?;
    ctx.verify_privilege(Action::PostFavorite)?;

    let user_id = ctx.client.id.ok_or(ApiError::NotLoggedIn)?;
    let new_post_favorite = NewPostFavorite { post_id, user_id };

    connection_pool
        .transaction(move |conn| {
            diesel::delete(post_favorite::table.find((post_id, user_id))).execute(conn)?;
            let insert_result = new_post_favorite.insert_into(post_favorite::table).execute(conn);
            error::map_foreign_key_violation(insert_result, ResourceType::Post)
        })
        .await?;
    connection_pool
        .transaction(move |conn| PostInfo::new_from_id(conn, &ctx, post_id, params.fields))
        .await
        .map(Json)
}

/// Updates score of authenticated user for given post.
#[utoipa::path(
    put,
    path = "/post/{id}/score",
    tag = POST_TAG,
    params(
        ("id" = i64, Path, description = "Post ID"),
        ResourceParams,
    ),
    request_body = RatingBody,
    responses(
        (status = 200, body = PostInfo),
        (status = 403, description = "Privileges are too low"),
        (status = 404, description = "Post does not exist"),
        (status = 422, description = "Score is invalid"),
    ),
)]
async fn rate(
    Ctx(ctx, connection_pool): Ctx,
    Path(post_id): Path<i64>,
    Query(params): Query<ResourceParams<Field>>,
    Json(body): Json<RatingBody>,
) -> ApiResult<Json<PostInfo>> {
    ctx.verify_privilege(Action::PostView)?;
    ctx.verify_privilege(Action::PostScore)?;

    let user_id = ctx.client.id.ok_or(ApiError::NotLoggedIn)?;
    connection_pool
        .transaction(move |conn| {
            diesel::delete(post_score::table.find((post_id, user_id))).execute(conn)?;

            if let Ok(score) = Score::try_from(*body) {
                let insert_result = NewPostScore {
                    post_id,
                    user_id,
                    score,
                }
                .insert_into(post_score::table)
                .execute(conn);
                error::map_foreign_key_violation(insert_result, ResourceType::Post)?;
            }
            Ok::<_, ApiError>(())
        })
        .await?;
    connection_pool
        .transaction(move |conn| PostInfo::new_from_id(conn, &ctx, post_id, params.fields))
        .await
        .map(Json)
}

async fn update_impl(
    ctx: Ctx,
    post_id: i64,
    params: ResourceParams<Field>,
    body: PostUpdateBody,
) -> ApiResult<Json<PostInfo>> {
    let new_content = match Content::new(body.content_token, body.content_url) {
        Some(content) => Some(content.remove_or_compute_properties(ctx.clone()).await?),
        None => None,
    };

    let custom_thumbnail = match Content::new(body.thumbnail_token, body.thumbnail_url) {
        Some(content) => Some(content.thumbnail(ctx.clone(), ThumbnailType::Post).await?),
        None => None,
    };

    let Ctx(ctx, connection_pool) = ctx;
    tagging_update(&connection_pool, body.tags.is_some(), {
        let ctx = ctx.clone();
        move |conn| {
            let old_post: Post = post::table
                .find(post_id)
                .first(conn)
                .optional()?
                .ok_or(ApiError::NotFound(ResourceType::Post))?;
            let old_mime_type = old_post.mime_type;
            api::verify_version(old_post.last_edit_time, body.version)?;

            let post_hash = PostHash::new(&ctx.config, post_id, Some(old_post.custom_thumbnail_size));

            let mut new_post = old_post.clone();
            let old_snapshot_data = SnapshotData::retrieve(conn, old_post)?;
            let mut new_snapshot_data = old_snapshot_data.clone();

            if let Some(safety) = body.safety {
                ctx.verify_privilege(Action::PostEditSafety)?;

                new_post.safety = safety;
                new_snapshot_data.safety = safety;
            }
            if let Some(flags) = body.flags {
                ctx.verify_privilege(Action::PostEditFlag)?;

                let updated_flags = PostFlags::from_slice(&flags);
                new_post.flags = updated_flags;
                new_snapshot_data.flags = updated_flags;
            }
            if let Some(source) = body.source {
                ctx.verify_privilege(Action::PostEditSource)?;

                new_post.source = source.clone();
                new_snapshot_data.source = source;
            }
            if let Some(description) = body.description {
                ctx.verify_privilege(Action::PostEditDescription)?;

                new_post.description = description.clone();
                new_snapshot_data.description = description;
            }
            if let Some(mut relations) = body.relations {
                ctx.verify_privilege(Action::PostEditRelation)?;

                update::post::set_relations(conn, &ctx, post_id, &mut relations)?;
                new_snapshot_data.relations = relations;
            }
            if let Some(tags) = body.tags {
                ctx.verify_privilege(Action::PostEditTag)?;

                let fetch_mode = if ctx.config.append_tag_implications_on_post_edit {
                    FetchMode::Deep
                } else {
                    FetchMode::Shallow
                };
                let (updated_tag_ids, tags) = update::tag::get_or_create_tags(conn, &ctx, tags, fetch_mode)?;
                update::post::set_tags(conn, post_id, &updated_tag_ids)?;
                new_snapshot_data.tags = tags;
            }
            if let Some(notes) = body.notes {
                ctx.verify_privilege(Action::PostEditNote)?;

                update::post::set_notes(conn, post_id, &notes)?;
                new_snapshot_data.notes = notes;
            }
            if let Some(content_properties) = new_content {
                ctx.verify_privilege(Action::PostEditContent)?;

                new_snapshot_data.checksum = hex::encode(&content_properties.checksum);

                // Update content metadata
                new_post.file_size = content_properties.file_size;
                new_post.width = content_properties.width;
                new_post.height = content_properties.height;
                new_post.type_ = content_properties.post_type;
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
                let temp_path = content_properties.token.path(&ctx.config);
                filesystem::delete_content(&post_hash, old_mime_type)?;
                filesystem::move_file(&temp_path, &post_hash.content_path(content_properties.mime_type))?;

                // Replace generated thumbnail
                update::post::thumbnail(conn, &post_hash, &content_properties.thumbnail, ThumbnailCategory::Generated)?;
            }
            if let Some(thumbnail) = custom_thumbnail {
                ctx.verify_privilege(Action::PostEditThumbnail)?;
                update::post::thumbnail(conn, &post_hash, &thumbnail, ThumbnailCategory::Custom)?;
            }

            new_post.last_edit_time = DateTime::now();
            let _: Post = error::map_unique_violation(new_post.save_changes(conn), ResourceProperty::PostContent)?;
            snapshot::post::modification_snapshot(conn, ctx.client, post_id, old_snapshot_data, new_snapshot_data)?;
            Ok(())
        }
    })
    .await?;
    connection_pool
        .transaction(move |conn| PostInfo::new_from_id(conn, &ctx, post_id, params.fields))
        .await
        .map(Json)
}

/// Request body for updating a post.
#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct PostUpdateBody {
    // Resource version. See [versioning](#Versioning).
    version: DateTime,
    /// Post safety rating.
    safety: Option<PostSafety>,
    /// Source URL or description.
    source: Option<LargeString>,
    /// Post description.
    description: Option<LargeString>,
    /// IDs of related posts.
    relations: Option<Vec<i64>>,
    /// Tags to apply. Non-existent tags will be created automatically.
    tags: Option<Vec<SmallString>>,
    /// Post annotations.
    notes: Option<Vec<Note>>,
    /// Post flags: `loop` or `sound`.
    flags: Option<Vec<PostFlag>>,
    /// Token referencing previously uploaded content.
    #[schema(value_type = Option<String>)]
    content_token: Option<UploadToken>,
    /// URL to fetch content from.
    content_url: Option<Url>,
    /// Token referencing previously uploaded thumbnail.
    #[schema(value_type = Option<String>)]
    thumbnail_token: Option<UploadToken>,
    /// URL to fetch thumbnail from.
    thumbnail_url: Option<Url>,
}

/// Updates existing post.
///
/// If specified tags do not exist yet, they will be automatically created.
/// Tags created automatically have no implications, no suggestions, one name
/// and their category is set to the first tag category found. Safety must be
/// any of `safe`, `sketchy` or `unsafe`. Relations must contain valid post IDs.
/// `flag` can be either `loop` to enable looping for video posts or `sound` to
/// indicate sound. Sending empty `thumbnail` will reset the post thumbnail to
/// default. For details how to pass `content` and `thumbnail`, see
/// [file uploads](#Upload). All fields except `version` are optional -
/// update concerns only provided fields.
#[utoipa::path(
    put,
    path = "/post/{id}",
    tag = POST_TAG,
    params(
        ("id" = i64, Path, description = "Post ID"),
        ResourceParams,
    ),
    request_body(
        content(
            (PostUpdateBody = "application/json"),
            (Multipart<PostUpdateBody> = "multipart/form-data"),
        )
    ),
    responses(
        (status = 200, body = PostInfo),
        (status = 403, description = "Privileges are too low"),
        (status = 404, description = "Post does not exist"),
        (status = 404, description = "Relations refer to non-existing posts"),
        (status = 409, description = "Version is outdated"),
        (status = 409, description = "Post content already exists"),
        (status = 422, description = "A tag has an invalid name"),
        (status = 422, description = "Safety is invalid"),
        (status = 422, description = "Notes are invalid"),
        (status = 422, description = "Flags are invalid"),
    ),
)]
async fn update(
    ctx: Ctx,
    Path(post_id): Path<i64>,
    Query(params): Query<ResourceParams<Field>>,
    body: JsonOrMultipart<PostUpdateBody>,
) -> ApiResult<Json<PostInfo>> {
    ctx.verify_privilege(Action::PostView)?;

    match body {
        JsonOrMultipart::Json(payload) => update_impl(ctx, post_id, params, payload).await,
        JsonOrMultipart::Multipart(payload) => {
            let decoded_body = upload::extract(&ctx.config, payload, [PartName::Content, PartName::Thumbnail]).await?;
            let metadata = decoded_body.metadata.ok_or(ApiError::MissingMetadata)?;
            let mut post_update: PostUpdateBody = serde_json::from_slice(&metadata)?;
            let [content_token, thumbnail_token] = decoded_body.files;

            post_update.content_token = content_token;
            post_update.thumbnail_token = thumbnail_token;
            update_impl(ctx, post_id, params, post_update).await
        }
    }
}

/// Deletes existing post.
///
/// Related posts and tags are kept.
#[utoipa::path(
    delete,
    path = "/post/{id}",
    tag = POST_TAG,
    params(
        ("id" = i64, Path, description = "Post ID"),
    ),
    request_body = DeleteBody,
    responses(
        (status = 200, body = Object),
        (status = 403, description = "Privileges are too low"),
        (status = 404, description = "Post does not exist"),
        (status = 409, description = "Version is outdated"),
    ),
)]
async fn delete(
    Ctx(ctx, connection_pool): Ctx,
    Path(post_id): Path<i64>,
    Json(client_version): Json<DeleteBody>,
) -> ApiResult<Json<()>> {
    // Post relation cascade deletion can cause deadlocks when deleting related posts in quick
    // succession, so we lock an aysnchronous mutex when deleting if the post has any relations.
    static ANTI_DEADLOCK_MUTEX: LazyLock<AsyncMutex<()>> = LazyLock::new(|| AsyncMutex::new(()));

    ctx.verify_privilege(Action::PostDelete)?;

    let relation_count: i64 = post_statistics::table
        .find(post_id)
        .select(post_statistics::relation_count)
        .first(connection_pool.get().await?.as_mut())
        .optional()?
        .ok_or(ApiError::NotFound(ResourceType::Post))?;

    let _lock;
    if relation_count > 0 {
        _lock = ANTI_DEADLOCK_MUTEX.lock().await;
    }

    let (mime_type, custom_thumbnail_size) = connection_pool
        .transaction(move |conn| {
            let post: Post = post::table
                .find(post_id)
                .first(conn)
                .optional()?
                .ok_or(ApiError::NotFound(ResourceType::Post))?;
            api::verify_version(post.last_edit_time, *client_version)?;

            let mime_type = post.mime_type;
            let custom_thumbnail_size = post.custom_thumbnail_size;
            let post_data = SnapshotData::retrieve(conn, post)?;
            snapshot::post::deletion_snapshot(conn, ctx.client, post_id, post_data)?;

            diesel::delete(post::table.find(post_id)).execute(conn)?;
            Ok::<_, ApiError>((mime_type, custom_thumbnail_size))
        })
        .await?;
    if ctx.config.delete_source_files {
        let post_hash = PostHash::new(&ctx.config, post_id, Some(custom_thumbnail_size));
        filesystem::delete_post(&post_hash, mime_type)?;
    }
    Ok(Json(()))
}

/// Unmarks the post as favorite for authenticated user.
#[utoipa::path(
    delete,
    path = "/post/{id}/favorite",
    tag = POST_TAG,
    params(
        ("id" = i64, Path, description = "Post ID"),
        ResourceParams,
    ),
    responses(
        (status = 200, body = PostInfo),
        (status = 403, description = "Privileges are too low"),
        (status = 404, description = "Post does not exist"),
    ),
)]
async fn unfavorite(
    Ctx(ctx, connection_pool): Ctx,
    Path(post_id): Path<i64>,
    Query(params): Query<ResourceParams<Field>>,
) -> ApiResult<Json<PostInfo>> {
    ctx.verify_privilege(Action::PostView)?;
    ctx.verify_privilege(Action::PostFavorite)?;

    let user_id = ctx.client.id.ok_or(ApiError::NotLoggedIn)?;
    connection_pool
        .transaction({
            let ctx = ctx.clone();
            move |conn| {
                verify_visibility(conn, &ctx, post_id)?;
                diesel::delete(post_favorite::table.find((post_id, user_id)))
                    .execute(conn)
                    .map_err(ApiError::from)
            }
        })
        .await?;
    connection_pool
        .transaction(move |conn| PostInfo::new_from_id(conn, &ctx, post_id, params.fields))
        .await
        .map(Json)
}

#[cfg(test)]
mod test {
    use crate::api::error::ApiResult;
    use crate::filesystem::Directory;
    use crate::model::enums::{ResourceType, UserRank};
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
        const PARAMS: &str = "-sort:id&limit=40&fields=id";
        verify_response(&format!("{QUERY}=-sort:id&limit=40{FIELDS}"), "post/list/typical").await?;

        let filter_table = crate::search::post::filter_table();
        for token in Token::iter() {
            let filter = filter_table[token];
            let (sign, filter) = if filter.starts_with('-') {
                filter.split_at(1)
            } else {
                ("", filter)
            };
            let query = format!("{QUERY}={sign}{token}:{filter} {PARAMS}");
            let path = format!("post/list/{token}_filtered");
            verify_response(&query, &path).await?;

            let query = format!("{QUERY}=sort:{token} {PARAMS}");
            let path = format!("post/list/{token}_sorted");
            verify_response(&query, &path).await?;
        }
        verify_response(&format!("{QUERY}=-pool:Fantasy {PARAMS}"), "post/list/pool_name_filtered").await?;
        verify_response(&format!("{QUERY}=pool:*a*t* {PARAMS}"), "post/list/pool_name_wildcards_filtered").await?;
        verify_response(&format!("{QUERY}=special:liked {PARAMS}"), "post/list/liked_filtered").await?;
        verify_response(&format!("{QUERY}=special:disliked {PARAMS}"), "post/list/disliked_filtered").await?;
        verify_response(&format!("{QUERY}=special:tumbleweed {PARAMS}"), "post/list/tumbleweed_filtered").await
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

        verify_response(&format!("GET /post/{POST_ID}/?{FIELDS}"), "post/get/typical").await?;

        let new_last_edit_time = get_last_edit_time(&mut conn)?;
        assert_eq!(new_last_edit_time, last_edit_time);
        Ok(())
    }

    #[tokio::test]
    #[parallel]
    async fn get_neighbors() -> ApiResult<()> {
        const QUERY: &str = "around/?query=-sort:id";
        verify_response(&format!("GET /post/1/{QUERY}{FIELDS}"), "post/get_around/1").await?;
        verify_response(&format!("GET /post/4/{QUERY}{FIELDS}"), "post/get_around/4").await?;
        verify_response(&format!("GET /post/5/{QUERY}{FIELDS}"), "post/get_around/5").await
    }

    #[tokio::test]
    #[parallel]
    async fn get_featured() -> ApiResult<()> {
        verify_response(&format!("GET /featured-post/?{FIELDS}"), "post/get_featured/typical").await
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

        verify_response(&format!("POST /featured-post/?{FIELDS}"), "post/feature/typical").await?;

        let (new_feature_count, new_last_edit_time) = get_post_info(&mut conn)?;
        assert_eq!(new_feature_count, feature_count + 1);
        assert_eq!(new_last_edit_time, last_edit_time);

        let last_feature_time: DateTime = post_feature::table
            .select(post_feature::time)
            .order(post_feature::time.desc())
            .first(&mut conn)?;
        diesel::delete(post_feature::table)
            .filter(post_feature::post_id.eq(POST_ID))
            .filter(post_feature::time.eq(last_feature_time))
            .execute(&mut conn)?;
        Ok(())
    }

    #[tokio::test]
    #[parallel]
    async fn reverse_search() -> ApiResult<()> {
        simulate_upload("1_pixel.png", "upload_for_reverse_search.png")?;
        verify_response(&format!("POST /posts/reverse-search/?{FIELDS}"), "post/reverse_search/typical").await
    }

    #[tokio::test]
    #[serial]
    async fn create() -> ApiResult<()> {
        simulate_upload("1_pixel.png", "cool_post.png")?;
        verify_response(&format!("POST /posts/?{FIELDS}"), "post/create/typical").await?;

        let state = get_state();
        let post_path = state.config.path(Directory::Posts);
        let thumbnail_path = state.config.path(Directory::GeneratedThumbnails);

        assert!(post_path.exists());
        assert!(thumbnail_path.exists());

        simulate_upload("1_pixel.png", "duplicate.png")?;
        verify_response("POST /posts", "post/create/duplicate").await?;
        verify_response("PUT /post/1", "post/edit/duplicate").await?;

        reset_database();
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

        verify_response(&format!("POST /post-merge/?{FIELDS}"), "post/merge/typical").await?;

        let has_post: bool = diesel::select(exists(post::table.find(REMOVE_ID))).first(&mut conn)?;
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

        verify_response(&format!("POST /post/{POST_ID}/favorite/?{FIELDS}"), "post/favorite/typical").await?;

        let (new_favorite_count, new_admin_favorite_count, new_last_edit_time) = get_post_info(&mut conn)?;
        assert_eq!(new_favorite_count, favorite_count + 1);
        assert_eq!(new_admin_favorite_count, admin_favorite_count + 1);
        assert_eq!(new_last_edit_time, last_edit_time);

        verify_response(&format!("DELETE /post/{POST_ID}/favorite/?{FIELDS}"), "post/unfavorite/typical").await?;

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

        verify_response(&format!("PUT /post/{POST_ID}/score/?{FIELDS}"), "post/rate/like").await?;

        let (new_score, new_last_edit_time) = get_post_info(&mut conn)?;
        assert_eq!(new_score, score + 1);
        assert_eq!(new_last_edit_time, last_edit_time);

        verify_response(&format!("PUT /post/{POST_ID}/score/?{FIELDS}"), "post/rate/dislike").await?;

        let (new_score, new_last_edit_time) = get_post_info(&mut conn)?;
        assert_eq!(new_score, score - 1);
        assert_eq!(new_last_edit_time, last_edit_time);

        verify_response(&format!("PUT /post/{POST_ID}/score/?{FIELDS}"), "post/rate/remove").await?;

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

        verify_response(&format!("PUT /post/{POST_ID}/?{FIELDS}"), "post/edit/typical").await?;

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

        verify_response(&format!("PUT /post/{POST_ID}/?{FIELDS}"), "post/edit/restore").await?;

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

    #[tokio::test]
    #[serial]
    async fn preferences() -> ApiResult<()> {
        verify_response_with_user(
            UserRank::Anonymous,
            "GET /posts/?query=-sort:id&limit=9&fields=id,relations,relationCount",
            "post/list/with_preferences",
        )
        .await?;
        verify_response_with_user(UserRank::Anonymous, "GET /post/5", "post/get/with_preferences").await?;
        verify_response_with_user(UserRank::Anonymous, "GET /post/4/around", "post/get_around/blacklisted").await?;
        verify_response_with_user(
            UserRank::Anonymous,
            "GET /post/4/around/?fields=id,relations,relationCount",
            "post/get_around/with_preferences",
        )
        .await?;
        verify_response_with_user(
            UserRank::Anonymous,
            "GET /featured-post/?fields=id,relations,relationCount",
            "post/get_featured/with_preferences",
        )
        .await?;

        simulate_upload("1_pixel.png", "upload_for_reverse_search.png")?;
        verify_response_with_user(
            UserRank::Anonymous,
            "POST /posts/reverse-search/?fields=id,relations,relationCount",
            "post/reverse_search/with_preferences",
        )
        .await?;

        verify_response_with_user(
            UserRank::Anonymous,
            "PUT /post/1/?fields=id,relations,relationCount",
            "post/edit/with_preferences",
        )
        .await?;

        reset_database();
        Ok(())
    }

    #[tokio::test]
    #[parallel]
    async fn error() -> ApiResult<()> {
        verify_response("GET /post/99", "post/get/nonexistent").await?;
        verify_response("GET /post/99/around", "post/get_around/nonexistent").await?;
        verify_response("POST /featured-post", "post/feature/nonexistent").await?;
        verify_response("POST /post-merge", "post/merge/to_nonexistent").await?;
        verify_response("POST /post-merge", "post/merge/from_nonexistent").await?;
        verify_response("POST /post/99/favorite", "post/favorite/nonexistent").await?;
        verify_response("PUT /post/99/score", "post/rate/nonexistent").await?;
        verify_response("PUT /post/99", "post/edit/nonexistent").await?;
        verify_response("DELETE /post/99", "post/delete/nonexistent").await?;
        verify_response("DELETE /post/99/favorite", "post/unfavorite/nonexistent").await?;

        simulate_upload("1_pixel.png", "upload.png")?;
        verify_response("POST /posts/reverse-search", "post/reverse_search/invalid_token").await?;

        verify_response("POST /posts", "post/create/invalid_tag").await?;
        verify_response("POST /posts", "post/create/invalid_safety").await?;
        verify_response("POST /posts", "post/create/invalid_note").await?;
        verify_response("POST /posts", "post/create/invalid_flag").await?;
        verify_response("POST /posts", "post/create/invalid_content_token").await?;
        verify_response("POST /posts", "post/create/invalid_thumbnail_token").await?;
        verify_response("POST /posts", "post/create/missing_content").await?;
        verify_response("POST /posts", "post/create/duplicate_relation").await?;
        verify_response("POST /posts", "post/create/nonexistent_relation").await?;

        verify_response("PUT /post/1", "post/edit/invalid_tag").await?;
        verify_response("PUT /post/1", "post/edit/invalid_safety").await?;
        verify_response("PUT /post/1", "post/edit/invalid_note").await?;
        verify_response("PUT /post/1", "post/edit/invalid_flag").await?;
        verify_response("PUT /post/1", "post/edit/invalid_content_token").await?;
        verify_response("PUT /post/1", "post/edit/invalid_thumbnail_token").await?;
        verify_response("PUT /post/1", "post/edit/duplicate_relation").await?;
        verify_response("PUT /post/1", "post/edit/nonexistent_relation").await?;

        verify_response("PUT /post/1/score", "post/rate/invalid").await?;
        verify_response("POST /featured-post", "post/feature/double_feature").await?;
        verify_response("POST /post-merge", "post/merge/with_self").await?;

        reset_sequence(ResourceType::Post)
    }

    #[tokio::test]
    #[parallel]
    async fn unauthorized() -> ApiResult<()> {
        const USER: UserRank = UserRank::Regular;

        simulate_upload("1_pixel.png", "upload.png")?;
        verify_response_with_user(USER, "GET /posts?limit=1", "post/list/unauthorized").await?;
        verify_response_with_user(USER, "GET /post/1", "post/get/unauthorized").await?;
        verify_response_with_user(USER, "GET /post/1/around", "post/get_around/unauthorized").await?;
        verify_response_with_user(USER, "GET /featured-post", "post/get_featured/unauthorized").await?;
        verify_response_with_user(USER, "POST /featured-post", "post/feature/unauthorized").await?;
        verify_response_with_user(USER, "POST /posts/reverse-search", "post/reverse_search/unauthorized").await?;
        verify_response_with_user(USER, "POST /posts", "post/create/anonymous_unauthorized").await?;
        verify_response_with_user(USER, "POST /posts", "post/create/identified_unauthorized").await?;
        verify_response_with_user(USER, "POST /post-merge", "post/merge/unauthorized").await?;
        verify_response_with_user(USER, "POST /post/1/favorite", "post/favorite/unauthorized").await?;
        verify_response_with_user(USER, "PUT /post/1/score", "post/rate/unauthorized").await?;
        verify_response_with_user(USER, "PUT /post/1", "post/edit/content_unauthorized").await?;
        verify_response_with_user(USER, "PUT /post/1", "post/edit/description_unauthorized").await?;
        verify_response_with_user(USER, "PUT /post/1", "post/edit/flag_unauthorized").await?;
        verify_response_with_user(USER, "PUT /post/1", "post/edit/note_unauthorized").await?;
        verify_response_with_user(USER, "PUT /post/1", "post/edit/relation_unauthorized").await?;
        verify_response_with_user(USER, "PUT /post/1", "post/edit/safety_unauthorized").await?;
        verify_response_with_user(USER, "PUT /post/1", "post/edit/source_unauthorized").await?;
        verify_response_with_user(USER, "PUT /post/1", "post/edit/tag_unauthorized").await?;
        verify_response_with_user(USER, "PUT /post/1", "post/edit/thumbnail_unauthorized").await?;
        verify_response_with_user(USER, "DELETE /post/1", "post/delete/unauthorized").await?;
        verify_response_with_user(USER, "DELETE /post/1/favorite", "post/unfavorite/unauthorized").await?;

        // Ensure users can't get around lack of view privileges via other actions
        verify_response_with_user(USER, "GET /posts?limit=1", "post/list/view_unauthorized").await?;
        verify_response_with_user(USER, "POST /posts/reverse-search", "post/reverse_search/view_unauthorized").await?;
        verify_response_with_user(USER, "POST /post-merge", "post/merge/view_unauthorized").await?;
        verify_response_with_user(USER, "POST /post/1/favorite", "post/favorite/view_unauthorized").await?;
        verify_response_with_user(USER, "PUT /post/1/score", "post/rate/view_unauthorized").await?;
        verify_response_with_user(USER, "PUT /post/1", "post/edit/view_unauthorized").await?;
        verify_response_with_user(USER, "DELETE /post/1/favorite", "post/unfavorite/view_unauthorized").await?;

        // Ensure users can't download files from web without authorization
        verify_response_with_user(USER, "POST /posts", "post/create/download_content_unauthorized").await?;
        verify_response_with_user(USER, "POST /posts", "post/create/download_thumbnail_unauthorized").await?;
        verify_response_with_user(USER, "PUT /post/1", "post/edit/download_content_unauthorized").await?;
        verify_response_with_user(USER, "PUT /post/1", "post/edit/download_thumbnail_unauthorized").await
    }

    #[tokio::test]
    #[serial]
    async fn unicode_edge_cases() -> ApiResult<()> {
        simulate_upload("1_pixel.png", "upload.png")?;
        verify_response("POST /posts?fields=tags", "post/create/unicode_tag_clash").await?;
        verify_response("PUT /post/1?fields=tags", "post/edit/unicode_tag_clash").await?;

        reset_database();
        Ok(())
    }

    #[tokio::test]
    #[parallel]
    async fn malicious_token() -> ApiResult<()> {
        // Place a file outside of the temporary uploads directory
        simulate_upload("1_pixel.png", "../upload.png")?;

        // Test responses that attempt to access file outside temporary uploads directory
        simulate_upload("1_pixel.png", "cool_post.png")?;
        verify_response("POST /posts/reverse-search", "post/reverse_search/malicious_token").await?;
        verify_response("POST /posts", "post/create/malicious_content_token").await?;
        verify_response("POST /posts", "post/create/malicious_thumbnail_token").await?;
        verify_response("PUT /post/1", "post/edit/malicious_content_token").await?;
        verify_response("PUT /post/1", "post/edit/malicious_thumbnail_token").await
    }
}
