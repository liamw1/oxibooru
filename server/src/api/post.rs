use crate::api::{
    ApiResult, AuthResult, DeleteRequest, MergeRequest, PagedQuery, PagedResponse, RatingRequest, ResourceQuery,
};
use crate::content::hash::PostHash;
use crate::content::thumbnail::{ThumbnailCategory, ThumbnailType};
use crate::content::{cache, signature, thumbnail};
use crate::filesystem::Directory;
use crate::model::enums::{MimeType, PostFlag, PostFlags, PostSafety, PostType, ResourceType, Score};
use crate::model::post::{
    NewPost, NewPostFavorite, NewPostFeature, NewPostScore, NewPostSignature, Post, PostRelation, PostSignature,
};
use crate::resource::post::{FieldTable, Note, PostInfo};
use crate::schema::{comment, post, post_favorite, post_feature, post_relation, post_score, post_signature, post_tag};
use crate::time::DateTime;
use crate::{api, config, db, filesystem, resource, search, update};
use diesel::dsl::exists;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::LazyLock;
use tokio::sync::Mutex;
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
        .and(api::resource_query())
        .map(get_post)
        .map(api::Reply::from);
    let get_post_neighbors = warp::get()
        .and(warp::path!("post" / i32 / "around"))
        .and(api::auth())
        .and(api::resource_query())
        .map(get_post_neighbors)
        .map(api::Reply::from);
    let get_featured_post = warp::get()
        .and(warp::path!("featured-post"))
        .and(api::auth())
        .and(api::resource_query())
        .map(get_featured_post)
        .map(api::Reply::from);
    let feature_post = warp::post()
        .and(warp::path!("featured-post"))
        .and(api::auth())
        .and(api::resource_query())
        .and(warp::body::json())
        .map(feature_post)
        .map(api::Reply::from);
    let reverse_search = warp::post()
        .and(warp::path!("posts" / "reverse-search"))
        .and(api::auth())
        .and(api::resource_query())
        .and(warp::body::json())
        .map(reverse_search)
        .map(api::Reply::from);
    let create_post = warp::post()
        .and(warp::path!("posts"))
        .and(api::auth())
        .and(api::resource_query())
        .and(warp::body::json())
        .map(create_post)
        .map(api::Reply::from);
    let merge_posts = warp::post()
        .and(warp::path!("post-merge"))
        .and(api::auth())
        .and(api::resource_query())
        .and(warp::body::json())
        .map(merge_posts)
        .map(api::Reply::from);
    let favorite_post = warp::post()
        .and(warp::path!("post" / i32 / "favorite"))
        .and(api::auth())
        .and(api::resource_query())
        .map(favorite_post)
        .map(api::Reply::from);
    let rate_post = warp::put()
        .and(warp::path!("post" / i32 / "score"))
        .and(api::auth())
        .and(api::resource_query())
        .and(warp::body::json())
        .map(rate_post)
        .map(api::Reply::from);
    let update_post = warp::put()
        .and(warp::path!("post" / i32))
        .and(api::auth())
        .and(api::resource_query())
        .and(warp::body::json())
        .map(update_post)
        .map(api::Reply::from);
    let delete_post = warp::delete()
        .and(warp::path!("post" / i32))
        .and(api::auth())
        .and(warp::body::json())
        .then(delete_post)
        .map(api::Reply::from);
    let unfavorite_post = warp::delete()
        .and(warp::path!("post" / i32 / "favorite"))
        .and(api::auth())
        .and(api::resource_query())
        .map(unfavorite_post)
        .map(api::Reply::from);

    list_posts
        .or(get_post)
        .or(get_post_neighbors)
        .or(get_featured_post)
        .or(feature_post)
        .or(reverse_search)
        .or(create_post)
        .or(merge_posts)
        .or(favorite_post)
        .or(rate_post)
        .or(update_post)
        .or(delete_post)
        .or(unfavorite_post)
}

const MAX_POSTS_PER_PAGE: i64 = 50;
static ANTI_DEADLOCK_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn create_field_table(fields: Option<&str>) -> Result<FieldTable<bool>, Box<dyn std::error::Error>> {
    fields
        .map(resource::post::Field::create_table)
        .transpose()
        .map(|opt_table| opt_table.unwrap_or(FieldTable::filled(true)))
        .map_err(Box::from)
}

fn list_posts(auth: AuthResult, query: PagedQuery) -> ApiResult<PagedResponse<PostInfo>> {
    let client = auth?;
    query.bump_login(client.as_ref())?;
    api::verify_privilege(client.as_ref(), config::privileges().post_list)?;

    let client_id = client.map(|user| user.id);
    let offset = query.offset.unwrap_or(0);
    let limit = std::cmp::min(query.limit.get(), MAX_POSTS_PER_PAGE);
    let fields = create_field_table(query.fields())?;

    db::get_connection()?.transaction(|conn| {
        let mut search_criteria = search::post::parse_search_criteria(query.criteria())?;
        search_criteria.add_offset_and_limit(offset, limit);
        let count_query = search::post::build_query(client_id, &search_criteria)?;
        let sql_query = search::post::build_query(client_id, &search_criteria)?;

        let total = count_query.count().first(conn)?;
        let selected_posts: Vec<i32> = search::post::get_ordered_ids(conn, sql_query, &search_criteria)?;
        Ok(PagedResponse {
            query: query.query.query,
            offset,
            limit,
            total,
            results: PostInfo::new_batch_from_ids(conn, client_id, selected_posts, &fields)?,
        })
    })
}

fn get_post(post_id: i32, auth: AuthResult, query: ResourceQuery) -> ApiResult<PostInfo> {
    let client = auth?;
    query.bump_login(client.as_ref())?;
    api::verify_privilege(client.as_ref(), config::privileges().post_view)?;

    let fields = create_field_table(query.fields())?;
    let client_id = client.map(|user| user.id);

    db::get_connection()?.transaction(|conn| {
        let post_exists: bool = diesel::select(exists(post::table.find(post_id))).get_result(conn)?;
        if !post_exists {
            return Err(api::Error::NotFound(ResourceType::Post));
        }
        PostInfo::new_from_id(conn, client_id, post_id, &fields).map_err(api::Error::from)
    })
}

#[derive(Serialize)]
struct PostNeighbors {
    prev: Option<PostInfo>,
    next: Option<PostInfo>,
}

fn get_post_neighbors(post_id: i32, auth: AuthResult, query: ResourceQuery) -> ApiResult<PostNeighbors> {
    let client = auth?;
    query.bump_login(client.as_ref())?;
    api::verify_privilege(client.as_ref(), config::privileges().post_list)?;

    let client_id = client.map(|user| user.id);
    let fields = create_field_table(query.fields())?;
    let search_criteria = search::post::parse_search_criteria(query.criteria())?;

    db::get_connection()?.transaction(|conn| {
        // Optimized neighbor retrieval for the most common use case
        if search_criteria.has_no_sort() {
            let previous_post = search::post::build_query(client_id, &search_criteria)?
                .select(Post::as_select())
                .filter(post::id.gt(post_id))
                .order_by(post::id.asc())
                .first(conn)
                .optional()?;
            let prev = previous_post
                .map(|post| PostInfo::new(conn, client_id, post, &fields))
                .transpose()?;

            let next_post = search::post::build_query(client_id, &search_criteria)?
                .select(Post::as_select())
                .filter(post::id.lt(post_id))
                .order_by(post::id.desc())
                .first(conn)
                .optional()?;
            let next = next_post
                .map(|post| PostInfo::new(conn, client_id, post, &fields))
                .transpose()?;

            Ok(PostNeighbors { prev, next })
        } else {
            let sql_query = search::post::build_query(client_id, &search_criteria)?;
            let post_ids: Vec<i32> = search::post::get_ordered_ids(conn, sql_query, &search_criteria)?;
            let post_index = post_ids.iter().position(|&id| id == post_id);

            let prev_post_id = post_index.and_then(|index| post_ids.get(index - 1));
            let prev = prev_post_id
                .map(|&post_id| PostInfo::new_from_id(conn, client_id, post_id, &fields))
                .transpose()?;

            let next_post_id = post_index.and_then(|index| post_ids.get(index + 1));
            let next = next_post_id
                .map(|&post_id| PostInfo::new_from_id(conn, client_id, post_id, &fields))
                .transpose()?;

            Ok(PostNeighbors { prev, next })
        }
    })
}

fn get_featured_post(auth: AuthResult, query: ResourceQuery) -> ApiResult<Option<PostInfo>> {
    let client = auth?;
    query.bump_login(client.as_ref())?;
    api::verify_privilege(client.as_ref(), config::privileges().post_view_featured)?;

    let client_id = client.map(|user| user.id);
    let fields = create_field_table(query.fields())?;

    db::get_connection()?.transaction(|conn| {
        let featured_post_id: Option<i32> = post_feature::table
            .select(post_feature::post_id)
            .order_by(post_feature::time.desc())
            .first(conn)
            .optional()?;

        featured_post_id
            .map(|post_id| PostInfo::new_from_id(conn, client_id, post_id, &fields))
            .transpose()
            .map_err(api::Error::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PostFeature {
    id: i32,
}

fn feature_post(auth: AuthResult, query: ResourceQuery, post_feature: PostFeature) -> ApiResult<PostInfo> {
    let client = auth?;
    query.bump_login(client.as_ref())?;
    api::verify_privilege(client.as_ref(), config::privileges().post_feature)?;

    let fields = create_field_table(query.fields())?;
    let post_id = post_feature.id;
    let user_id = client.ok_or(api::Error::NotLoggedIn).map(|user| user.id)?;
    let new_post_feature = NewPostFeature { post_id, user_id };

    db::get_connection()?.transaction(|conn| {
        diesel::insert_into(post_feature::table)
            .values(new_post_feature)
            .execute(conn)?;

        PostInfo::new_from_id(conn, Some(user_id), post_id, &fields).map_err(api::Error::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
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
    let client = auth?;
    query.bump_login(client.as_ref())?;
    api::verify_privilege(client.as_ref(), config::privileges().post_reverse_search)?;

    let fields = create_field_table(query.fields())?;
    let content_properties = cache::compute_properties(token.content_token)?;

    let client_id = client.map(|user| user.id);
    db::get_connection()?.transaction(|conn| {
        // Check for exact match
        let exact_post = post::table
            .filter(post::checksum.eq(content_properties.checksum))
            .first(conn)
            .optional()?;
        if exact_post.is_some() {
            return Ok(ReverseSearchInfo {
                exact_post: exact_post
                    .map(|post_id| PostInfo::new(conn, client_id, post_id, &fields))
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
        Ok(ReverseSearchInfo {
            exact_post: None,
            similar_posts: PostInfo::new_batch_from_ids(conn, client_id, post_ids, &fields)?
                .into_iter()
                .zip(distances)
                .map(|(post, distance)| SimilarPostInfo { distance, post })
                .collect(),
        })
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct NewPostInfo {
    content_token: String,
    thumbnail_token: Option<String>,
    safety: PostSafety,
    source: Option<String>,
    relations: Option<Vec<i32>>,
    anonymous: Option<bool>,
    tags: Option<Vec<String>>,
    flags: Option<Vec<PostFlag>>,
}

fn create_post(auth: AuthResult, query: ResourceQuery, post_info: NewPostInfo) -> ApiResult<PostInfo> {
    let required_rank = match post_info.anonymous.unwrap_or(false) {
        true => config::privileges().post_create_anonymous,
        false => config::privileges().post_create_identified,
    };
    let client = auth?;
    query.bump_login(client.as_ref())?;
    api::verify_privilege(client.as_ref(), required_rank)?;

    let fields = create_field_table(query.fields())?;
    let content_properties = cache::get_or_compute_properties(post_info.content_token)?;
    let custom_thumbnail = post_info
        .thumbnail_token
        .map(|token| thumbnail::create_from_token(&token, ThumbnailType::Post))
        .transpose()?;

    let mut flags = content_properties.flags;
    for flag in post_info.flags.unwrap_or_default() {
        flags.add(flag);
    }

    let client_id = client.as_ref().map(|user| user.id);
    let new_post = NewPost {
        user_id: client_id,
        file_size: content_properties.file_size as i64,
        width: content_properties.width as i32,
        height: content_properties.height as i32,
        safety: post_info.safety,
        type_: PostType::from(content_properties.mime_type),
        mime_type: content_properties.mime_type,
        checksum: &content_properties.checksum,
        checksum_md5: &content_properties.md5_checksum,
        flags,
        source: post_info.source.as_deref(),
    };

    db::get_connection()?.transaction(|conn| {
        let post_id = diesel::insert_into(post::table)
            .values(new_post)
            .returning(post::id)
            .get_result(conn)?;
        let post_hash = PostHash::new(post_id);

        // Add tags
        let tags =
            update::tag::get_or_create_tag_ids(conn, client.as_ref(), post_info.tags.unwrap_or_default(), false)?;
        update::post::add_tags(conn, post_id, tags)?;

        // Add relations
        let relations = post_info.relations.unwrap_or_default();
        update::post::create_relations(conn, post_id, relations)?;

        // Create post signature
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
            api::verify_privilege(client.as_ref(), config::privileges().post_edit_thumbnail)?;
            filesystem::save_post_thumbnail(&post_hash, thumbnail, ThumbnailCategory::Custom)?;
        }
        filesystem::save_post_thumbnail(&post_hash, content_properties.thumbnail, ThumbnailCategory::Generated)?;

        PostInfo::new_from_id(conn, client_id, post_id, &fields).map_err(api::Error::from)
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PostMergeRequest {
    #[serde(flatten)]
    post_info: MergeRequest<i32>,
    replace_content: bool,
}

fn merge_posts(auth: AuthResult, query: ResourceQuery, merge_info: PostMergeRequest) -> ApiResult<PostInfo> {
    let client = auth?;
    query.bump_login(client.as_ref())?;
    api::verify_privilege(client.as_ref(), config::privileges().post_merge)?;

    let client_id = client.as_ref().map(|user| user.id);
    let remove_id = merge_info.post_info.remove;
    let merge_to_id = merge_info.post_info.merge_to;
    if remove_id == merge_to_id {
        return Err(api::Error::SelfMerge(ResourceType::Post));
    }
    let remove_hash = PostHash::new(remove_id);
    let merge_to_hash = PostHash::new(merge_to_id);

    let fields = create_field_table(query.fields())?;
    db::get_connection()?.transaction(|conn| {
        let mut remove_post: Post = post::table.find(remove_id).first(conn)?;
        let mut merge_to_post: Post = post::table.find(merge_to_id).first(conn)?;
        api::verify_version(remove_post.last_edit_time, merge_info.post_info.remove_version)?;
        api::verify_version(merge_to_post.last_edit_time, merge_info.post_info.merge_to_version)?;

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
        diesel::update(post_tag::table)
            .filter(post_tag::post_id.eq(remove_id))
            .filter(post_tag::tag_id.ne_all(merge_to_tags))
            .set(post_tag::post_id.eq(merge_to_id))
            .execute(conn)?;

        // Merge scores
        let merge_to_scores = post_score::table
            .select(post_score::user_id)
            .filter(post_score::post_id.eq(merge_to_id))
            .into_boxed();
        diesel::update(post_score::table)
            .filter(post_score::post_id.eq(remove_id))
            .filter(post_score::user_id.ne_all(merge_to_scores))
            .set(post_score::post_id.eq(merge_to_id))
            .execute(conn)?;

        // Merge favorites
        let merge_to_favorites = post_favorite::table
            .select(post_favorite::user_id)
            .filter(post_favorite::post_id.eq(merge_to_id))
            .into_boxed();
        diesel::update(post_favorite::table)
            .filter(post_favorite::post_id.eq(remove_id))
            .filter(post_favorite::user_id.ne_all(merge_to_favorites))
            .set(post_favorite::post_id.eq(merge_to_id))
            .execute(conn)?;

        // Merge features
        let merge_to_features = post_feature::table
            .select(post_feature::id)
            .filter(post_feature::post_id.eq(merge_to_id))
            .into_boxed();
        diesel::update(post_feature::table)
            .filter(post_feature::post_id.eq(remove_id))
            .filter(post_feature::id.ne_all(merge_to_features))
            .set(post_feature::post_id.eq(merge_to_id))
            .execute(conn)?;

        // Merge comments
        let merge_to_comments = comment::table
            .select(comment::id)
            .filter(comment::post_id.eq(merge_to_id))
            .into_boxed();
        diesel::update(comment::table)
            .filter(comment::post_id.eq(remove_id))
            .filter(comment::id.ne_all(merge_to_comments))
            .set(comment::post_id.eq(merge_to_id))
            .execute(conn)?;

        // If replacing content, update post signature. This needs to be done before deletion because post signatures cascade
        if merge_info.replace_content {
            let (signature, indexes): (Vec<Option<i64>>, Vec<Option<i32>>) = post_signature::table
                .find(remove_id)
                .select((post_signature::signature, post_signature::words))
                .first(conn)?;
            diesel::update(post_signature::table.find(merge_to_id))
                .set((post_signature::signature.eq(signature), post_signature::words.eq(indexes)))
                .execute(conn)?;
        }

        diesel::delete(post::table.find(remove_id)).execute(conn)?;

        if merge_info.replace_content {
            filesystem::swap_posts(&remove_hash, remove_post.mime_type, &merge_to_hash, merge_to_post.mime_type)?;

            // If replacing content, update metadata. This needs to be done after deletion because checksum has UNIQUE constraint
            std::mem::swap(&mut remove_post.user_id, &mut merge_to_post.user_id);
            std::mem::swap(&mut remove_post.file_size, &mut merge_to_post.file_size);
            std::mem::swap(&mut remove_post.width, &mut merge_to_post.width);
            std::mem::swap(&mut remove_post.height, &mut merge_to_post.height);
            std::mem::swap(&mut remove_post.type_, &mut merge_to_post.type_);
            std::mem::swap(&mut remove_post.mime_type, &mut merge_to_post.mime_type);
            std::mem::swap(&mut remove_post.checksum, &mut merge_to_post.checksum);
            std::mem::swap(&mut remove_post.checksum_md5, &mut merge_to_post.checksum_md5);
            std::mem::swap(&mut remove_post.flags, &mut merge_to_post.flags);
            std::mem::swap(&mut remove_post.source, &mut merge_to_post.source);
            merge_to_post = merge_to_post.save_changes(conn)?;
        }

        if config::get().delete_source_files {
            // This is the correct id and mime_type, even if replacing content :)
            filesystem::delete_post(&remove_hash, remove_post.mime_type)?;
        }

        PostInfo::new(conn, client_id, merge_to_post, &fields).map_err(api::Error::from)
    })
}

fn favorite_post(post_id: i32, auth: AuthResult, query: ResourceQuery) -> ApiResult<PostInfo> {
    let client = auth?;
    query.bump_login(client.as_ref())?;
    api::verify_privilege(client.as_ref(), config::privileges().post_favorite)?;

    let fields = create_field_table(query.fields())?;
    let user_id = client.ok_or(api::Error::NotLoggedIn).map(|user| user.id)?;
    let new_post_favorite = NewPostFavorite { post_id, user_id };

    db::get_connection()?.transaction(|conn| {
        diesel::delete(post_favorite::table.find((post_id, user_id))).execute(conn)?;
        diesel::insert_into(post_favorite::table)
            .values(new_post_favorite)
            .execute(conn)?;

        PostInfo::new_from_id(conn, Some(user_id), post_id, &fields).map_err(api::Error::from)
    })
}

fn rate_post(post_id: i32, auth: AuthResult, query: ResourceQuery, rating: RatingRequest) -> ApiResult<PostInfo> {
    let client = auth?;
    query.bump_login(client.as_ref())?;
    api::verify_privilege(client.as_ref(), config::privileges().post_score)?;

    let fields = create_field_table(query.fields())?;
    let user_id = client.ok_or(api::Error::NotLoggedIn).map(|user| user.id)?;

    db::get_connection()?.transaction(|conn| {
        diesel::delete(post_score::table.find((post_id, user_id))).execute(conn)?;

        if let Ok(score) = Score::try_from(*rating) {
            let new_post_score = NewPostScore {
                post_id,
                user_id,
                score,
            };
            diesel::insert_into(post_score::table)
                .values(new_post_score)
                .execute(conn)?;
        }

        PostInfo::new_from_id(conn, Some(user_id), post_id, &fields).map_err(api::Error::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct PostUpdate {
    version: DateTime,
    safety: Option<PostSafety>,
    source: Option<String>,
    relations: Option<Vec<i32>>,
    tags: Option<Vec<String>>,
    notes: Option<Vec<Note>>,
    flags: Option<Vec<PostFlag>>,
    content_token: Option<String>,
    thumbnail_token: Option<String>,
}

fn update_post(post_id: i32, auth: AuthResult, query: ResourceQuery, update: PostUpdate) -> ApiResult<PostInfo> {
    let client = auth?;
    query.bump_login(client.as_ref())?;
    let fields = create_field_table(query.fields())?;
    let new_content = update.content_token.map(cache::get_or_compute_properties).transpose()?;
    let custom_thumbnail = update
        .thumbnail_token
        .map(|token| thumbnail::create_from_token(&token, ThumbnailType::Post))
        .transpose()?;

    let post_hash = PostHash::new(post_id);
    db::get_connection()?.transaction(|conn| {
        let post_version = post::table.find(post_id).select(post::last_edit_time).first(conn)?;
        api::verify_version(post_version, update.version)?;

        if let Some(safety) = update.safety {
            api::verify_privilege(client.as_ref(), config::privileges().post_edit_safety)?;

            diesel::update(post::table.find(post_id))
                .set(post::safety.eq(safety))
                .execute(conn)?;
        }
        if let Some(source) = update.source {
            api::verify_privilege(client.as_ref(), config::privileges().post_edit_source)?;

            diesel::update(post::table.find(post_id))
                .set(post::source.eq(source))
                .execute(conn)?;
        }
        if let Some(relations) = update.relations {
            api::verify_privilege(client.as_ref(), config::privileges().post_edit_relation)?;

            update::post::delete_relations(conn, post_id)?;
            update::post::create_relations(conn, post_id, relations)?;
        }
        if let Some(tags) = update.tags {
            api::verify_privilege(client.as_ref(), config::privileges().post_edit_tag)?;

            let updated_tag_ids = update::tag::get_or_create_tag_ids(conn, client.as_ref(), tags, false)?;
            update::post::delete_tags(conn, post_id)?;
            update::post::add_tags(conn, post_id, updated_tag_ids)?;
        }
        if let Some(notes) = update.notes {
            api::verify_privilege(client.as_ref(), config::privileges().post_edit_note)?;

            update::post::delete_notes(conn, post_id)?;
            update::post::add_notes(conn, post_id, notes)?;
        }
        if let Some(flags) = update.flags {
            api::verify_privilege(client.as_ref(), config::privileges().post_edit_flag)?;

            let updated_flags = PostFlags::from_slice(&flags);
            diesel::update(post::table.find(post_id))
                .set(post::flags.eq(updated_flags))
                .execute(conn)?;
        }
        if let Some(content_properties) = new_content {
            api::verify_privilege(client.as_ref(), config::privileges().post_edit_content)?;

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
            filesystem::save_post_thumbnail(&post_hash, content_properties.thumbnail, ThumbnailCategory::Generated)?;
        }
        if let Some(thumbnail) = custom_thumbnail {
            api::verify_privilege(client.as_ref(), config::privileges().post_edit_thumbnail)?;

            filesystem::delete_post_thumbnail(&post_hash, ThumbnailCategory::Custom)?;
            filesystem::save_post_thumbnail(&post_hash, thumbnail, ThumbnailCategory::Custom)?;
        }

        let client_id = client.map(|user| user.id);
        PostInfo::new_from_id(conn, client_id, post_id, &fields).map_err(api::Error::from)
    })
}

async fn delete_post(post_id: i32, auth: AuthResult, client_version: DeleteRequest) -> ApiResult<()> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().post_delete)?;

    let mut conn = db::get_connection()?;
    let relation_count: i64 = post_relation::table
        .filter(post_relation::parent_id.eq(post_id))
        .or_filter(post_relation::child_id.eq(post_id))
        .count()
        .first(&mut conn)?;

    let mut delete_and_get_mime_type = || -> ApiResult<MimeType> {
        conn.transaction(|conn| {
            let (mime_type, post_version) = post::table
                .find(post_id)
                .select((post::mime_type, post::last_edit_time))
                .first(conn)?;
            api::verify_version(post_version, *client_version)?;

            diesel::delete(post::table.find(post_id)).execute(conn)?;
            Ok(mime_type)
        })
    };

    // Post relation cascade deletion can cause deadlocks when deleting related posts in quick
    // succession, so we lock an aysnchronous mutex when deleting if the post has any relations.
    let mime_type = if relation_count > 0 {
        let _lock = ANTI_DEADLOCK_MUTEX.lock().await;
        delete_and_get_mime_type()
    } else {
        delete_and_get_mime_type()
    }?;

    if config::get().delete_source_files {
        filesystem::delete_post(&PostHash::new(post_id), mime_type)?;
    }
    Ok(())
}

fn unfavorite_post(post_id: i32, auth: AuthResult, query: ResourceQuery) -> ApiResult<PostInfo> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().post_favorite)?;

    let fields = create_field_table(query.fields())?;
    let user_id = client.ok_or(api::Error::NotLoggedIn).map(|user| user.id)?;

    db::get_connection()?.transaction(|conn| {
        diesel::delete(post_favorite::table.find((post_id, user_id))).execute(conn)?;
        PostInfo::new_from_id(conn, Some(user_id), post_id, &fields).map_err(api::Error::from)
    })
}
