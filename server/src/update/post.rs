use crate::api::ApiResult;
use crate::config::Config;
use crate::content::hash::PostHash;
use crate::content::thumbnail::ThumbnailCategory;
use crate::filesystem;
use crate::model::comment::NewComment;
use crate::model::pool::PoolPost;
use crate::model::post::{
    CompressedSignature, NewPostFeature, Post, PostFavorite, PostRelation, PostScore, PostTag, SignatureIndexes,
};
use crate::resource::post::Note;
use crate::schema::{
    comment, pool_post, post, post_favorite, post_feature, post_note, post_relation, post_score, post_signature,
    post_tag,
};
use crate::time::DateTime;
use diesel::{ExpressionMethods, Insertable, PgConnection, QueryDsl, QueryResult, RunQueryDsl};
use image::DynamicImage;
use std::collections::HashSet;

/// Updates `last_edit_time` of post associated with `post_id`.
pub fn last_edit_time(conn: &mut PgConnection, post_id: i64) -> ApiResult<()> {
    diesel::update(post::table.find(post_id))
        .set(post::last_edit_time.eq(DateTime::now()))
        .execute(conn)?;
    Ok(())
}

/// Updates thumbnail for post.
pub fn thumbnail(
    conn: &mut PgConnection,
    post_hash: &PostHash,
    thumbnail: &DynamicImage,
    thumbnail_type: ThumbnailCategory,
) -> ApiResult<()> {
    filesystem::delete_post_thumbnail(post_hash, thumbnail_type)?;
    let thumbnail_size = filesystem::save_post_thumbnail(post_hash, thumbnail, thumbnail_type)?;
    match thumbnail_type {
        ThumbnailCategory::Generated => diesel::update(post::table.find(post_hash.id()))
            .set(post::generated_thumbnail_size.eq(thumbnail_size))
            .execute(conn)?,
        ThumbnailCategory::Custom => diesel::update(post::table.find(post_hash.id()))
            .set(post::custom_thumbnail_size.eq(thumbnail_size))
            .execute(conn)?,
    };
    Ok(())
}

/// Replaces the current set of relations with `relations` for post associated with `post_id`.
pub fn set_relations(conn: &mut PgConnection, post_id: i64, new_related_posts: &[i64]) -> QueryResult<()> {
    let new_relations: Vec<_> = new_related_posts
        .iter()
        .filter(|&&id| id != post_id)
        .flat_map(|&other_id| PostRelation::new_pair(post_id, other_id))
        .collect();

    // Delete old relations and return old related posts ids.
    // Post relations are bi-directional, so it doesn't matter whether we return parent_id or child_id.
    let old_related_posts: Vec<i64> = diesel::delete(post_relation::table)
        .filter(post_relation::parent_id.eq(post_id))
        .or_filter(post_relation::child_id.eq(post_id))
        .returning(post_relation::child_id)
        .get_results(conn)?;
    new_relations.insert_into(post_relation::table).execute(conn)?;

    // Update last edit time for any posts involved in removed or added relations.
    let updated_posts: Vec<_> = old_related_posts
        .iter()
        .chain(new_related_posts)
        .filter(|&&id| id != post_id)
        .collect();
    diesel::update(post::table)
        .set(post::last_edit_time.eq(DateTime::now()))
        .filter(post::id.eq_any(updated_posts))
        .execute(conn)?;
    Ok(())
}

/// Replaces the current set of tags with `tags` for post associated with `post_id`.
pub fn set_tags(conn: &mut PgConnection, post_id: i64, tags: &[i64]) -> QueryResult<()> {
    let new_post_tags: Vec<_> = tags.iter().map(|&tag_id| PostTag { post_id, tag_id }).collect();

    diesel::delete(post_tag::table)
        .filter(post_tag::post_id.eq(post_id))
        .execute(conn)?;
    new_post_tags.insert_into(post_tag::table).execute(conn)?;
    Ok(())
}

/// Replaces the current set of notes with `notes` for post associated with `post_id`.
pub fn set_notes(conn: &mut PgConnection, post_id: i64, notes: &[Note]) -> QueryResult<()> {
    let new_post_notes: Vec<_> = notes.iter().map(|note| note.to_new_post_note(post_id)).collect();

    diesel::delete(post_note::table)
        .filter(post_note::post_id.eq(post_id))
        .execute(conn)?;
    new_post_notes.insert_into(post_note::table).execute(conn)?;
    Ok(())
}

/// Merges `absorbed_post` to `merge_to_post`.
pub fn merge(
    conn: &mut PgConnection,
    config: &Config,
    absorbed_post: &Post,
    merge_to_post: &Post,
    replace_content: bool,
) -> ApiResult<()> {
    let absorbed_id = absorbed_post.id;
    let merge_to_id = merge_to_post.id;
    let absorbed_hash = PostHash::new(config, absorbed_id);
    let merge_to_hash = PostHash::new(config, merge_to_id);

    // Merge relations
    let involved_relations: Vec<PostRelation> = post_relation::table
        .filter(post_relation::parent_id.eq(absorbed_id))
        .or_filter(post_relation::child_id.eq(absorbed_id))
        .or_filter(post_relation::parent_id.eq(merge_to_id))
        .or_filter(post_relation::child_id.eq(merge_to_id))
        .load(conn)?;
    let merged_relations: HashSet<_> = involved_relations
        .iter()
        .copied()
        .map(|mut relation| {
            if relation.parent_id == absorbed_id {
                relation.parent_id = merge_to_id;
            } else if relation.child_id == absorbed_id {
                relation.child_id = merge_to_id;
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
        .filter(post_tag::post_id.eq(absorbed_id))
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
        .filter(pool_post::post_id.eq(absorbed_id))
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
        .filter(post_score::post_id.eq(absorbed_id))
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
        .filter(post_favorite::post_id.eq(absorbed_id))
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
    let new_features: Vec<_> = diesel::delete(post_feature::table.filter(post_feature::post_id.eq(absorbed_id)))
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
    let removed_comments: Vec<(_, String, _)> = diesel::delete(comment::table.filter(comment::post_id.eq(absorbed_id)))
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
    let merged_description = merge_to_post.description.to_string() + "\n\n" + &absorbed_post.description;
    diesel::update(post::table.find(merge_to_id))
        .set(post::description.eq(merged_description.trim()))
        .execute(conn)?;

    // If replacing content, update post signature. This needs to be done before deletion because post signatures cascade
    if replace_content {
        let (signature, indexes): (CompressedSignature, SignatureIndexes) = post_signature::table
            .find(absorbed_id)
            .select((post_signature::signature, post_signature::words))
            .first(conn)?;
        diesel::update(post_signature::table.find(merge_to_id))
            .set((post_signature::signature.eq(signature), post_signature::words.eq(indexes)))
            .execute(conn)?;
    }

    diesel::delete(post::table.find(absorbed_id)).execute(conn)?;

    if replace_content {
        filesystem::swap_posts(
            config,
            &absorbed_hash,
            absorbed_post.mime_type,
            &merge_to_hash,
            merge_to_post.mime_type,
        )?;

        // If replacing content, update metadata. This needs to be done after deletion because checksum has UNIQUE constraint
        diesel::update(post::table.find(merge_to_post.id))
            .set((
                post::file_size.eq(absorbed_post.file_size),
                post::width.eq(absorbed_post.width),
                post::height.eq(absorbed_post.height),
                post::type_.eq(absorbed_post.type_),
                post::mime_type.eq(absorbed_post.mime_type),
                post::checksum.eq(&absorbed_post.checksum),
                post::checksum_md5.eq(&absorbed_post.checksum_md5),
                post::flags.eq(absorbed_post.flags),
                post::source.eq(&absorbed_post.source),
                post::generated_thumbnail_size.eq(absorbed_post.generated_thumbnail_size),
                post::custom_thumbnail_size.eq(absorbed_post.custom_thumbnail_size),
            ))
            .execute(conn)?;
    }

    if config.delete_source_files {
        let deleted_content_type = if replace_content {
            merge_to_post.mime_type
        } else {
            absorbed_post.mime_type
        };
        filesystem::delete_post(&absorbed_hash, deleted_content_type)?;
    }
    last_edit_time(conn, merge_to_id)?;
    Ok(())
}
