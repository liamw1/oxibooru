use crate::api::{ApiResult, AuthResult, DeleteBody, MergeBody, PageParams, PagedResponse, RatingBody, ResourceParams};
use crate::content::hash::PostHash;
use crate::content::thumbnail::{ThumbnailCategory, ThumbnailType};
use crate::content::upload::{PartName, MAX_UPLOAD_SIZE};
use crate::content::{signature, upload, Content, FileContents};
use crate::filesystem::Directory;
use crate::model::comment::NewComment;
use crate::model::enums::{PostFlag, PostFlags, PostSafety, PostType, ResourceType, Score};
use crate::model::pool::PoolPost;
use crate::model::post::{
    NewPost, NewPostFeature, NewPostSignature, Post, PostFavorite, PostRelation, PostScore, PostSignature, PostTag,
};
use crate::resource::post::{Note, PostInfo};
use crate::schema::{
    comment, database_statistics, pool_post, post, post_favorite, post_feature, post_relation, post_score,
    post_signature, post_statistics, post_tag,
};
use crate::time::DateTime;
use crate::{api, config, db, filesystem, resource, search, update};
use diesel::dsl::exists;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::LazyLock;
use tokio::sync::Mutex as AsyncMutex;
use url::Url;
use warp::multipart::FormData;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list = warp::get()
        .and(api::auth())
        .and(warp::path!("posts"))
        .and(warp::query())
        .map(list)
        .map(api::Reply::from);
    let get = warp::get()
        .and(api::auth())
        .and(warp::path!("post" / i64))
        .and(api::resource_query())
        .map(get)
        .map(api::Reply::from);
    let get_neighbors = warp::get()
        .and(api::auth())
        .and(warp::path!("post" / i64 / "around"))
        .and(api::resource_query())
        .map(get_neighbors)
        .map(api::Reply::from);
    let get_featured = warp::get()
        .and(api::auth())
        .and(warp::path!("featured-post"))
        .and(api::resource_query())
        .map(get_featured)
        .map(api::Reply::from);
    let feature = warp::post()
        .and(api::auth())
        .and(warp::path!("featured-post"))
        .and(api::resource_query())
        .and(warp::body::json())
        .map(feature)
        .map(api::Reply::from);
    let reverse_search = warp::post()
        .and(api::auth())
        .and(warp::path!("posts" / "reverse-search"))
        .and(api::resource_query())
        .and(warp::body::json())
        .then(reverse_search)
        .map(api::Reply::from);
    let reverse_search_multipart = warp::post()
        .and(api::auth())
        .and(warp::path!("posts" / "reverse-search"))
        .and(api::resource_query())
        .and(warp::filters::multipart::form().max_length(MAX_UPLOAD_SIZE))
        .then(reverse_search_multipart)
        .map(api::Reply::from);
    let create = warp::post()
        .and(api::auth())
        .and(warp::path!("posts"))
        .and(api::resource_query())
        .and(warp::body::json())
        .then(create)
        .map(api::Reply::from);
    let create_multipart = warp::post()
        .and(api::auth())
        .and(warp::path!("posts"))
        .and(api::resource_query())
        .and(warp::filters::multipart::form().max_length(MAX_UPLOAD_SIZE))
        .then(create_multipart)
        .map(api::Reply::from);
    let merge = warp::post()
        .and(api::auth())
        .and(warp::path!("post-merge"))
        .and(api::resource_query())
        .and(warp::body::json())
        .map(merge)
        .map(api::Reply::from);
    let favorite = warp::post()
        .and(api::auth())
        .and(warp::path!("post" / i64 / "favorite"))
        .and(api::resource_query())
        .map(favorite)
        .map(api::Reply::from);
    let rate = warp::put()
        .and(api::auth())
        .and(warp::path!("post" / i64 / "score"))
        .and(api::resource_query())
        .and(warp::body::json())
        .map(rate)
        .map(api::Reply::from);
    let update = warp::put()
        .and(api::auth())
        .and(warp::path!("post" / i64))
        .and(api::resource_query())
        .and(warp::body::json())
        .then(update)
        .map(api::Reply::from);
    let update_multipart = warp::put()
        .and(api::auth())
        .and(warp::path!("post" / i64))
        .and(api::resource_query())
        .and(warp::filters::multipart::form().max_length(MAX_UPLOAD_SIZE))
        .then(update_multipart)
        .map(api::Reply::from);
    let delete = warp::delete()
        .and(api::auth())
        .and(warp::path!("post" / i64))
        .and(warp::body::json())
        .then(delete)
        .map(api::Reply::from);
    let unfavorite = warp::delete()
        .and(api::auth())
        .and(warp::path!("post" / i64 / "favorite"))
        .and(api::resource_query())
        .map(unfavorite)
        .map(api::Reply::from);

    list.or(get)
        .or(get_neighbors)
        .or(get_featured)
        .or(feature)
        .or(reverse_search)
        .or(reverse_search_multipart)
        .or(create)
        .or(create_multipart)
        .or(merge)
        .or(favorite)
        .or(rate)
        .or(update)
        .or(update_multipart)
        .or(delete)
        .or(unfavorite)
}

const MAX_POSTS_PER_PAGE: i64 = 1000;

fn list(auth: AuthResult, params: PageParams) -> ApiResult<PagedResponse<PostInfo>> {
    let client = auth?;
    params.bump_login(client)?;
    api::verify_privilege(client, config::privileges().post_list)?;

    let offset = params.offset.unwrap_or(0);
    let limit = std::cmp::min(params.limit.get(), MAX_POSTS_PER_PAGE);
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    db::get_connection()?.transaction(|conn| {
        let mut search_criteria = search::post::parse_search_criteria(params.criteria())?;
        search_criteria.add_offset_and_limit(offset, limit);
        let sql_query = search::post::build_query(client, &search_criteria)?;

        let total = if search_criteria.has_filter() {
            let count_query = search::post::build_query(client, &search_criteria)?;
            count_query.count().first(conn)?
        } else {
            database_statistics::table
                .select(database_statistics::post_count)
                .first(conn)?
        };

        let selected_posts: Vec<i64> = search::post::get_ordered_ids(conn, sql_query, &search_criteria)?;
        Ok(PagedResponse {
            query: params.to_query(),
            offset,
            limit,
            total,
            results: PostInfo::new_batch_from_ids(conn, client, selected_posts, &fields)?,
        })
    })
}

fn get(auth: AuthResult, post_id: i64, params: ResourceParams) -> ApiResult<PostInfo> {
    let client = auth?;
    params.bump_login(client)?;
    api::verify_privilege(client, config::privileges().post_view)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    db::get_connection()?.transaction(|conn| {
        let post_exists: bool = diesel::select(exists(post::table.find(post_id))).get_result(conn)?;
        if !post_exists {
            return Err(api::Error::NotFound(ResourceType::Post));
        }
        PostInfo::new_from_id(conn, client, post_id, &fields).map_err(api::Error::from)
    })
}

#[derive(Serialize)]
struct PostNeighbors {
    prev: Option<PostInfo>,
    next: Option<PostInfo>,
}

fn get_neighbors(auth: AuthResult, post_id: i64, params: ResourceParams) -> ApiResult<PostNeighbors> {
    let client = auth?;
    params.bump_login(client)?;
    api::verify_privilege(client, config::privileges().post_list)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let search_criteria = search::post::parse_search_criteria(params.criteria())?;

    let create_post_neighbors = |mut neighbors: Vec<PostInfo>, has_previous_post: bool| {
        let (prev, next) = match (neighbors.pop(), neighbors.pop()) {
            (Some(second), Some(first)) => (Some(first), Some(second)),
            (Some(first), None) if has_previous_post => (Some(first), None),
            (Some(first), None) => (None, Some(first)),
            (None, Some(_)) => unreachable!(),
            (None, None) => (None, None),
        };
        PostNeighbors { prev, next }
    };

    db::get_connection()?.transaction(|conn| {
        if search_criteria.has_sort() {
            // Most general method of retrieving neighbors
            let sql_query = search::post::build_query(client, &search_criteria)?;
            let post_ids: Vec<i64> = search::post::get_ordered_ids(conn, sql_query, &search_criteria)?;
            let post_index = post_ids.iter().position(|&id| id == post_id);

            let prev_post_id = post_index
                .and_then(|index| index.checked_sub(1))
                .and_then(|prev_index| post_ids.get(prev_index))
                .copied();
            let next_post_id = post_index
                .and_then(|index| index.checked_add(1))
                .and_then(|next_index| post_ids.get(next_index))
                .copied();
            let post_ids = prev_post_id.into_iter().chain(next_post_id).collect();
            let post_infos = PostInfo::new_batch_from_ids(conn, client, post_ids, &fields)?;
            Ok(create_post_neighbors(post_infos, prev_post_id.is_some()))
        } else {
            // Optimized neighbor retrieval for the most common use case
            let previous_post = search::post::build_query(client, &search_criteria)?
                .select(Post::as_select())
                .filter(post::id.gt(post_id))
                .order_by(post::id.asc())
                .first(conn)
                .optional()?;
            let next_post = search::post::build_query(client, &search_criteria)?
                .select(Post::as_select())
                .filter(post::id.lt(post_id))
                .order_by(post::id.desc())
                .first(conn)
                .optional()?;

            let has_previous_post = previous_post.is_some();
            let posts = previous_post.into_iter().chain(next_post).collect();
            let post_infos = PostInfo::new_batch(conn, client, posts, &fields)?;
            Ok(create_post_neighbors(post_infos, has_previous_post))
        }
    })
}

fn get_featured(auth: AuthResult, params: ResourceParams) -> ApiResult<Option<PostInfo>> {
    let client = auth?;
    params.bump_login(client)?;
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
            .map_err(api::Error::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FeatureBody {
    id: i64,
}

fn feature(auth: AuthResult, params: ResourceParams, body: FeatureBody) -> ApiResult<PostInfo> {
    let client = auth?;
    params.bump_login(client)?;
    api::verify_privilege(client, config::privileges().post_feature)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let user_id = client.id.ok_or(api::Error::NotLoggedIn)?;
    let new_post_feature = NewPostFeature {
        post_id: body.id,
        user_id,
        time: DateTime::now(),
    };

    let mut conn = db::get_connection()?;
    diesel::insert_into(post_feature::table)
        .values(new_post_feature)
        .execute(&mut conn)?;
    conn.transaction(|conn| PostInfo::new_from_id(conn, client, body.id, &fields).map_err(api::Error::from))
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
    auth: AuthResult,
    params: ResourceParams,
    body: ReverseSearchBody,
) -> ApiResult<ReverseSearchResponse> {
    let _timer = crate::time::Timer::new("reverse search");
    let client = auth?;
    params.bump_login(client)?;
    api::verify_privilege(client, config::privileges().post_reverse_search)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let content = Content::new(body.content, body.content_token, body.content_url)
        .ok_or(api::Error::MissingContent(ResourceType::Post))?;
    let content_properties = content.compute_properties().await?;
    db::get_connection()?.transaction(|conn| {
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

        // Search for similar images
        let similar_signatures =
            PostSignature::find_similar(conn, signature::generate_indexes(content_properties.signature))?;
        println!("Found {} similar signatures", similar_signatures.len());
        let mut similar_posts: Vec<_> = similar_signatures
            .into_iter()
            .filter_map(|post_signature| {
                let distance = signature::distance(
                    content_properties.signature,
                    signature::from_database(post_signature.signature),
                );
                let distance_threshold = 1.0 - config::get().post_similarity_threshold;
                (distance < distance_threshold).then_some((post_signature.post_id, distance))
            })
            .collect();
        similar_posts.sort_unstable_by(|(_, dist_a), (_, dist_b)| dist_a.partial_cmp(dist_b).unwrap());

        let (post_ids, distances): (Vec<_>, Vec<_>) = similar_posts.into_iter().unzip();
        Ok(ReverseSearchResponse {
            exact_post: None,
            similar_posts: PostInfo::new_batch_from_ids(conn, client, post_ids, &fields)?
                .into_iter()
                .zip(distances)
                .map(|(post, distance)| SimilarPost { distance, post })
                .collect(),
        })
    })
}

async fn reverse_search_multipart(
    auth: AuthResult,
    params: ResourceParams,
    form_data: FormData,
) -> ApiResult<ReverseSearchResponse> {
    let body = upload::extract_without_metadata(form_data, [PartName::Content]).await?;
    if let [Some(content)] = body.files {
        let body = ReverseSearchBody {
            content: Some(content),
            content_token: None,
            content_url: None,
        };
        reverse_search(auth, params, body).await
    } else {
        Err(api::Error::MissingFormData)
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
    relations: Option<Vec<i64>>,
    anonymous: Option<bool>,
    tags: Option<Vec<String>>,
    notes: Option<Vec<Note>>,
    flags: Option<Vec<PostFlag>>,
}

async fn create(auth: AuthResult, params: ResourceParams, body: CreateBody) -> ApiResult<PostInfo> {
    let client = auth?;
    let required_rank = match body.anonymous.unwrap_or(false) {
        true => config::privileges().post_create_anonymous,
        false => config::privileges().post_create_identified,
    };
    params.bump_login(client)?;
    api::verify_privilege(client, required_rank)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let content = Content::new(body.content, body.content_token, body.content_url)
        .ok_or(api::Error::MissingContent(ResourceType::Post))?;
    let content_properties = content.get_or_compute_properties().await?;

    let custom_thumbnail = match Content::new(body.thumbnail, body.thumbnail_token, body.thumbnail_url) {
        Some(content) => Some(content.thumbnail(ThumbnailType::Post).await?),
        None => None,
    };

    let mut flags = content_properties.flags;
    for flag in body.flags.unwrap_or_default() {
        flags.add(flag);
    }

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
        source: body.source.as_deref(),
    };

    let mut conn = db::get_connection()?;
    let post_id = conn.transaction(|conn| {
        let post_id = diesel::insert_into(post::table)
            .values(new_post)
            .returning(post::id)
            .get_result(conn)?;
        let post_hash = PostHash::new(post_id);

        // Add tags, relations, and notes
        if let Some(tags) = body.tags {
            let tag_ids = update::tag::get_or_create_tag_ids(conn, client, tags, false)?;
            update::post::add_tags(conn, post_id, tag_ids)?;
        }
        if let Some(relations) = body.relations {
            update::post::create_relations(conn, post_id, relations)?;
        }
        if let Some(notes) = body.notes {
            update::post::add_notes(conn, post_id, notes)?;
        }

        let new_post_signature = NewPostSignature {
            post_id,
            signature: &content_properties.signature,
            words: &signature::generate_indexes(content_properties.signature),
        };
        diesel::insert_into(post_signature::table)
            .values(new_post_signature)
            .execute(conn)?;

        // Move content to permanent location
        let temp_path = filesystem::temporary_upload_filepath(&content_properties.token);
        filesystem::create_dir(Directory::Posts)?;
        std::fs::rename(temp_path, post_hash.content_path(content_properties.mime_type))?;

        // Create thumbnails
        if let Some(thumbnail) = custom_thumbnail {
            api::verify_privilege(client, config::privileges().post_edit_thumbnail)?;
            update::post::custom_thumbnail(conn, &post_hash, thumbnail)?;
        }
        let generated_thumbnail_size =
            filesystem::save_post_thumbnail(&post_hash, content_properties.thumbnail, ThumbnailCategory::Generated)?;
        diesel::update(post::table.find(post_id))
            .set(post::generated_thumbnail_size.eq(generated_thumbnail_size as i64))
            .execute(conn)?;
        Ok::<_, api::Error>(post_id)
    })?;
    conn.transaction(|conn| PostInfo::new_from_id(conn, client, post_id, &fields).map_err(api::Error::from))
}

async fn create_multipart(auth: AuthResult, params: ResourceParams, form_data: FormData) -> ApiResult<PostInfo> {
    let body = upload::extract_with_metadata(form_data, [PartName::Content, PartName::Thumbnail]).await?;
    let metadata = body.metadata.ok_or(api::Error::MissingMetadata)?;
    let mut new_post: CreateBody = serde_json::from_slice(&metadata)?;
    let [content, thumbnail] = body.files;

    new_post.content = content;
    new_post.thumbnail = thumbnail;
    create(auth, params, new_post).await
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct PostMergeBody {
    #[serde(flatten)]
    post_info: MergeBody<i64>,
    replace_content: bool,
}

fn merge(auth: AuthResult, params: ResourceParams, body: PostMergeBody) -> ApiResult<PostInfo> {
    let client = auth?;
    params.bump_login(client)?;
    api::verify_privilege(client, config::privileges().post_merge)?;

    let remove_id = body.post_info.remove;
    let merge_to_id = body.post_info.merge_to;
    if remove_id == merge_to_id {
        return Err(api::Error::SelfMerge(ResourceType::Post));
    }
    let remove_hash = PostHash::new(remove_id);
    let merge_to_hash = PostHash::new(merge_to_id);

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let mut conn = db::get_connection()?;
    let merged_post = conn.transaction(|conn| {
        let remove_post: Post = post::table.find(remove_id).first(conn)?;
        let mut merge_to_post: Post = post::table.find(merge_to_id).first(conn)?;
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
        diesel::insert_into(post_relation::table)
            .values(merged_relations.into_iter().collect::<Vec<_>>())
            .execute(conn)?;

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
        diesel::insert_into(post_tag::table).values(new_tags).execute(conn)?;

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
        diesel::insert_into(pool_post::table).values(new_pools).execute(conn)?;

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
        diesel::insert_into(post_score::table)
            .values(new_scores)
            .execute(conn)?;

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
        diesel::insert_into(post_favorite::table)
            .values(new_favorites)
            .execute(conn)?;

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
        diesel::insert_into(post_feature::table)
            .values(new_features)
            .execute(conn)?;

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
        diesel::insert_into(comment::table).values(new_comments).execute(conn)?;

        // If replacing content, update post signature. This needs to be done before deletion because post signatures cascade
        if body.replace_content && !cfg!(test) {
            let (signature, indexes): (Vec<Option<i64>>, Vec<Option<i32>>) = post_signature::table
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
            merge_to_post.file_size = remove_post.file_size;
            merge_to_post.width = remove_post.width;
            merge_to_post.height = remove_post.height;
            merge_to_post.type_ = remove_post.type_;
            merge_to_post.mime_type = remove_post.mime_type;
            merge_to_post.checksum = remove_post.checksum;
            merge_to_post.checksum_md5 = remove_post.checksum_md5;
            merge_to_post.flags = remove_post.flags;
            merge_to_post.source = remove_post.source;
            merge_to_post.generated_thumbnail_size = remove_post.generated_thumbnail_size;
            merge_to_post.custom_thumbnail_size = remove_post.custom_thumbnail_size;
            merge_to_post = merge_to_post.save_changes(conn)?;
        }

        if config::get().delete_source_files && !cfg!(test) {
            // This is the correct id and mime_type, even if replacing content :)
            filesystem::delete_post(&remove_hash, remove_post.mime_type)?;
        }
        update::post::last_edit_time(conn, merge_to_id).map(|_| merge_to_post)
    })?;
    conn.transaction(|conn| PostInfo::new(conn, client, merged_post, &fields).map_err(api::Error::from))
}

fn favorite(auth: AuthResult, post_id: i64, params: ResourceParams) -> ApiResult<PostInfo> {
    let client = auth?;
    params.bump_login(client)?;
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
        diesel::insert_into(post_favorite::table)
            .values(new_post_favorite)
            .execute(conn)
            .map_err(api::Error::from)
    })?;
    conn.transaction(|conn| PostInfo::new_from_id(conn, client, post_id, &fields).map_err(api::Error::from))
}

fn rate(auth: AuthResult, post_id: i64, params: ResourceParams, body: RatingBody) -> ApiResult<PostInfo> {
    let client = auth?;
    params.bump_login(client)?;
    api::verify_privilege(client, config::privileges().post_score)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let user_id = client.id.ok_or(api::Error::NotLoggedIn)?;

    let mut conn = db::get_connection()?;
    conn.transaction(|conn| {
        diesel::delete(post_score::table.find((post_id, user_id))).execute(conn)?;

        if let Ok(score) = Score::try_from(*body) {
            let new_post_score = PostScore {
                post_id,
                user_id,
                score,
                time: DateTime::now(),
            };
            diesel::insert_into(post_score::table)
                .values(new_post_score)
                .execute(conn)?;
        }
        Ok::<_, api::Error>(())
    })?;
    conn.transaction(|conn| PostInfo::new_from_id(conn, client, post_id, &fields).map_err(api::Error::from))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct UpdateBody {
    version: DateTime,
    safety: Option<PostSafety>,
    #[serde(default, deserialize_with = "api::deserialize_some")]
    source: Option<Option<String>>,
    relations: Option<Vec<i64>>,
    tags: Option<Vec<String>>,
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

async fn update(auth: AuthResult, post_id: i64, params: ResourceParams, body: UpdateBody) -> ApiResult<PostInfo> {
    let client = auth?;
    params.bump_login(client)?;

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

    // Updating tags of many posts simultaneously can cause deadlocks due to statistics updating,
    // so we serialize tag updates. Technically this is more pessimistic than necessary. Parallel
    // updates are fine if the sets of tags are disjoint, but implementing this is more complicated
    // so we just do this for now.
    static ANTI_DEADLOCK_MUTEX: LazyLock<AsyncMutex<()>> = LazyLock::new(|| AsyncMutex::new(()));
    {
        let _lock;
        if body.tags.is_some() {
            _lock = ANTI_DEADLOCK_MUTEX.lock().await;
        }

        db::get_connection()?.transaction(|conn| {
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
            if let Some(relations) = body.relations {
                api::verify_privilege(client, config::privileges().post_edit_relation)?;

                update::post::delete_relations(conn, post_id)?;
                update::post::create_relations(conn, post_id, relations)?;
            }
            if let Some(tags) = body.tags {
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
                    signature: &content_properties.signature,
                    words: &signature::generate_indexes(content_properties.signature),
                };
                diesel::delete(post_signature::table.find(post_id)).execute(conn)?;
                diesel::insert_into(post_signature::table)
                    .values(new_post_signature)
                    .execute(conn)?;

                // Replace content
                let temp_path = filesystem::temporary_upload_filepath(&content_properties.token);
                filesystem::delete_content(&post_hash, old_mime_type)?;
                std::fs::rename(temp_path, post_hash.content_path(content_properties.mime_type))?;

                // Replace generated thumbnail
                filesystem::delete_post_thumbnail(&post_hash, ThumbnailCategory::Generated)?;
                let generated_thumbnail_size = filesystem::save_post_thumbnail(
                    &post_hash,
                    content_properties.thumbnail,
                    ThumbnailCategory::Generated,
                )?;
                diesel::update(post::table.find(post_id))
                    .set(post::generated_thumbnail_size.eq(generated_thumbnail_size as i64))
                    .execute(conn)?;
            }
            if let Some(thumbnail) = custom_thumbnail {
                api::verify_privilege(client, config::privileges().post_edit_thumbnail)?;
                update::post::custom_thumbnail(conn, &post_hash, thumbnail)?;
            }
            update::post::last_edit_time(conn, post_id)
        })?;
    }
    db::get_connection()?
        .transaction(|conn| PostInfo::new_from_id(conn, client, post_id, &fields).map_err(api::Error::from))
}

async fn update_multipart(
    auth: AuthResult,
    post_id: i64,
    params: ResourceParams,
    form_data: FormData,
) -> ApiResult<PostInfo> {
    let body = upload::extract_with_metadata(form_data, [PartName::Content, PartName::Thumbnail]).await?;
    let metadata = body.metadata.ok_or(api::Error::MissingMetadata)?;
    let mut post_update: UpdateBody = serde_json::from_slice(&metadata)?;
    let [content, thumbnail] = body.files;

    post_update.content = content;
    post_update.thumbnail = thumbnail;
    update(auth, post_id, params, post_update).await
}

async fn delete(auth: AuthResult, post_id: i64, client_version: DeleteBody) -> ApiResult<()> {
    let client = auth?;
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
    Ok(())
}

fn unfavorite(auth: AuthResult, post_id: i64, params: ResourceParams) -> ApiResult<PostInfo> {
    let client = auth?;
    api::verify_privilege(client, config::privileges().post_favorite)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let user_id = client.id.ok_or(api::Error::NotLoggedIn)?;

    let mut conn = db::get_connection()?;
    diesel::delete(post_favorite::table.find((post_id, user_id))).execute(&mut conn)?;
    conn.transaction(|conn| PostInfo::new_from_id(conn, client, post_id, &fields).map_err(api::Error::from))
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
    flags,source,contentUrl,thumbnailUrl,tags,relations,pools,notes,score,ownScore,ownFavorite,tagCount,commentCount,\
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
        assert_eq!(new_post.creation_time, post.creation_time);
        assert!(new_post.last_edit_time > post.last_edit_time);
        assert_eq!(new_tag_count, tag_count);
        assert_eq!(new_relation_count, relation_count);
        Ok(())
    }
}
