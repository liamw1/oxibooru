use crate::api::{
    ApiResult, AuthResult, DeleteRequest, MergeRequest, PagedQuery, PagedResponse, RatingRequest, ResourceQuery,
};
use crate::auth::content;
use crate::image::{read, signature};
use crate::model::enums::{MimeType, PostSafety, PostType, Score};
use crate::model::post::{NewPost, NewPostFavorite, NewPostFeature, NewPostScore, NewPostSignature, PostSignature};
use crate::resource::post::{FieldTable, PostInfo};
use crate::schema::{comment, post, post_favorite, post_feature, post_relation, post_score, post_signature, post_tag};
use crate::util::DateTime;
use crate::{api, config, filesystem, resource, search, update, util};
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
        .map(delete_post)
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
const POST_SIMILARITY_THRESHOLD: f64 = 0.45;

fn create_field_table(fields: Option<&str>) -> Result<FieldTable<bool>, Box<dyn std::error::Error>> {
    fields
        .map(resource::post::Field::create_table)
        .transpose()
        .map(|opt_table| opt_table.unwrap_or(FieldTable::filled(true)))
        .map_err(Box::from)
}

fn list_posts(auth: AuthResult, query: PagedQuery) -> ApiResult<PagedResponse<PostInfo>> {
    let _timer = crate::util::Timer::new("list_posts");

    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().post_list)?;

    let client_id = client.map(|user| user.id);
    let offset = query.offset.unwrap_or(0);
    let limit = std::cmp::min(query.limit.get(), MAX_POSTS_PER_PAGE);
    let fields = create_field_table(query.fields())?;

    crate::get_connection()?.transaction(|conn| {
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
    api::verify_privilege(client.as_ref(), config::privileges().post_view)?;

    let fields = create_field_table(query.fields())?;
    let client_id = client.map(|user| user.id);

    crate::get_connection()?
        .transaction(|conn| PostInfo::new_from_id(conn, client_id, post_id, &fields).map_err(api::Error::from))
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
    let search_criteria = search::post::parse_search_criteria(query.criteria())?;
    let sql_query = search::post::build_query(client_id, &search_criteria)?;

    crate::get_connection()?.transaction(|conn| {
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
    })
}

fn get_featured_post(auth: AuthResult, query: ResourceQuery) -> ApiResult<Option<PostInfo>> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().post_view_featured)?;

    let client_id = client.map(|user| user.id);
    let fields = create_field_table(query.fields())?;

    crate::get_connection()?.transaction(|conn| {
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
    api::verify_privilege(client.as_ref(), config::privileges().post_feature)?;

    let fields = create_field_table(query.fields())?;
    let post_id = post_feature.id;
    let user_id = client.ok_or(api::Error::NotLoggedIn).map(|user| user.id)?;
    let new_post_feature = NewPostFeature { post_id, user_id };

    crate::get_connection()?.transaction(|conn| {
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
    let image_size = std::fs::metadata(&temp_path)?.len();
    let image_reader = read::new_image_reader(&temp_path)?;
    let image = image_reader.decode()?;
    let image_signature = signature::compute_signature(&image);
    let image_checksum = content::image_checksum(&image, image_size);

    let client_id = client.map(|user| user.id);
    crate::get_connection()?.transaction(|conn| {
        // Check for exact match
        let exact_post = post::table
            .filter(post::checksum.eq(image_checksum))
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
        let similar_signatures = PostSignature::find_similar(conn, signature::generate_indexes(&image_signature))?;
        println!("Found {} similar signatures", similar_signatures.len());
        let mut similar_posts: Vec<_> = similar_signatures
            .into_iter()
            .filter_map(|post_signature| {
                let distance = signature::normalized_distance(&post_signature.signature, &image_signature);
                (distance < POST_SIMILARITY_THRESHOLD).then_some((post_signature.post_id, distance))
            })
            .collect();
        similar_posts.sort_unstable_by(|(_, dist_a), (_, dist_b)| dist_a.partial_cmp(dist_b).unwrap());

        let post_ids = similar_posts.iter().map(|(post_id, _)| *post_id).collect();
        let distances: Vec<f64> = similar_posts.iter().map(|(_, distance)| *distance).collect();
        Ok(ReverseSearchInfo {
            exact_post: None,
            similar_posts: PostInfo::new_batch_from_ids(conn, client_id, post_ids, &fields)?
                .into_iter()
                .zip(distances.into_iter())
                .map(|(post, distance)| SimilarPostInfo { distance, post })
                .collect(),
        })
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct NewPostInfo {
    safety: PostSafety,
    source: Option<String>,
    relations: Option<Vec<i32>>,
    anonymous: Option<bool>,
    content_token: String,
    tags: Option<Vec<String>>,
    // flags: TODO
}

fn create_post(auth: AuthResult, query: ResourceQuery, post_info: NewPostInfo) -> ApiResult<PostInfo> {
    let _timer = crate::util::Timer::new("create_post");

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
    let image_reader = read::new_image_reader(&temp_path)?;
    let image = image_reader.decode()?;
    let image_checksum = content::image_checksum(&image, image_size);

    let client_id = client.as_ref().map(|user| user.id);
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

    crate::get_connection()?.transaction(|conn| {
        let (post_id, mime_type) = diesel::insert_into(post::table)
            .values(new_post)
            .returning((post::id, post::mime_type))
            .get_result(conn)?;

        // Add tags
        let tags = update::tag::get_or_create_tag_ids(conn, client.as_ref(), post_info.tags.unwrap_or_default())?;
        update::post::add_tags(conn, post_id, tags)?;

        // Add relations
        let relations = post_info.relations.unwrap_or_default();
        update::post::create_relations(conn, post_id, relations)?;

        // Generate image signature
        let image_signature = signature::compute_signature(&image);
        let new_post_signature = NewPostSignature {
            post_id,
            signature: &image_signature,
            words: &signature::generate_indexes(&image_signature),
        };
        diesel::insert_into(post_signature::table)
            .values(new_post_signature)
            .execute(conn)?;

        // Move content to permanent location
        let posts_folder = filesystem::posts_directory();
        if !posts_folder.exists() {
            std::fs::create_dir(posts_folder)?;
        }
        std::fs::rename(temp_path, content::post_content_path(post_id, mime_type))?;

        // Generate thumbnail
        let thumbnail_folder = filesystem::generated_thumbnails_directory();
        if !thumbnail_folder.exists() {
            std::fs::create_dir(thumbnail_folder)?;
        }
        let thumbnail = image.resize_to_fill(
            config::get().thumbnails.post_width,
            config::get().thumbnails.post_height,
            image::imageops::FilterType::Gaussian,
        );
        thumbnail.to_rgb8().save(content::post_thumbnail_path(post_id))?;

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
    let _timer = crate::util::Timer::new("merge_posts");

    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().post_merge)?;

    let client_id = client.as_ref().map(|user| user.id);
    let remove_id = merge_info.post_info.remove;
    let merge_to_id = merge_info.post_info.merge_to;
    if remove_id == merge_to_id {
        return Err(api::Error::SelfMerge);
    }

    let fields = create_field_table(query.fields())?;
    let merged_post = crate::get_connection()?.transaction(|conn| {
        let remove_version = post::table.find(remove_id).select(post::last_edit_time).first(conn)?;
        let merge_to_version = post::table.find(merge_to_id).select(post::last_edit_time).first(conn)?;
        api::verify_version(remove_version, merge_info.post_info.remove_version)?;
        api::verify_version(merge_to_version, merge_info.post_info.merge_to_version)?;

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

        // Merge relations
        let merge_to_relations = post_relation::table
            .select(post_relation::child_id)
            .filter(post_relation::parent_id.eq(merge_to_id))
            .into_boxed();
        diesel::update(post_relation::table)
            .filter(post_relation::parent_id.eq(remove_id))
            .filter(post_relation::child_id.ne_all(merge_to_relations))
            .set(post_relation::parent_id.eq(merge_to_id))
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

        diesel::delete(post::table.find(remove_id)).execute(conn)?;
        PostInfo::new_from_id(conn, client_id, merge_to_id, &fields).map_err(api::Error::from)
    })?;

    if merge_info.replace_content {
        // TODO
    }
    if config::get().delete_source_files {
        // TODO
    }

    Ok(merged_post)
}

fn favorite_post(post_id: i32, auth: AuthResult, query: ResourceQuery) -> ApiResult<PostInfo> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().post_favorite)?;

    let fields = create_field_table(query.fields())?;
    let user_id = client.ok_or(api::Error::NotLoggedIn).map(|user| user.id)?;
    let new_post_favorite = NewPostFavorite { post_id, user_id };

    crate::get_connection()?.transaction(|conn| {
        diesel::delete(post_favorite::table.find((post_id, user_id))).execute(conn)?;
        diesel::insert_into(post_favorite::table)
            .values(new_post_favorite)
            .execute(conn)?;

        PostInfo::new_from_id(conn, Some(user_id), post_id, &fields).map_err(api::Error::from)
    })
}

fn rate_post(post_id: i32, auth: AuthResult, query: ResourceQuery, rating: RatingRequest) -> ApiResult<PostInfo> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().post_score)?;

    let fields = create_field_table(query.fields())?;
    let user_id = client.ok_or(api::Error::NotLoggedIn).map(|user| user.id)?;

    crate::get_connection()?.transaction(|conn| {
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
struct PostUpdate {
    version: DateTime,
    safety: Option<PostSafety>,
    source: Option<String>,
    relations: Option<Vec<i32>>,
    tags: Option<Vec<String>>,
    // notes: TODO
    // flags: TODO
}

fn update_post(post_id: i32, auth: AuthResult, query: ResourceQuery, update: PostUpdate) -> ApiResult<PostInfo> {
    let _timer = crate::util::Timer::new("update_post");

    let client = auth?;
    let fields = create_field_table(query.fields())?;

    crate::get_connection()?.transaction(|conn| {
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

            let updated_tag_ids = update::tag::get_or_create_tag_ids(conn, client.as_ref(), tags)?;
            update::post::delete_tags(conn, post_id)?;
            update::post::add_tags(conn, post_id, updated_tag_ids)?;
        }

        let client_id = client.map(|user| user.id);
        PostInfo::new_from_id(conn, client_id, post_id, &fields).map_err(api::Error::from)
    })
}

/*
    Deletes the post with the specified ID. Uses deadlock_prone_transaction because
    post relation cascade deletion causes deadlocks when deleting related posts
    in quick succession.
*/
fn delete_post(post_id: i32, auth: AuthResult, client_version: DeleteRequest) -> ApiResult<()> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().post_delete)?;

    let mut conn = crate::get_connection()?;
    let mime_type = util::deadlock_prone_transaction::<_, api::Error, _>(&mut conn, 3, |conn| {
        let (mime_type, post_version) = post::table
            .find(post_id)
            .select((post::mime_type, post::last_edit_time))
            .first(conn)?;
        api::verify_version(post_version, *client_version)?;

        diesel::delete(post::table.find(post_id)).execute(conn)?;
        Ok(mime_type)
    })?;

    if config::get().delete_source_files {
        filesystem::remove_post(post_id, mime_type)?;
    }
    Ok(())
}

fn unfavorite_post(post_id: i32, auth: AuthResult, query: ResourceQuery) -> ApiResult<PostInfo> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().post_favorite)?;

    let fields = create_field_table(query.fields())?;
    let user_id = client.ok_or(api::Error::NotLoggedIn).map(|user| user.id)?;

    crate::get_connection()?.transaction(|conn| {
        diesel::delete(post_favorite::table.find((post_id, user_id))).execute(conn)?;
        PostInfo::new_from_id(conn, Some(user_id), post_id, &fields).map_err(api::Error::from)
    })
}
