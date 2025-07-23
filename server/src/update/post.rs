use crate::api::ApiResult;
use crate::content::hash::PostHash;
use crate::content::thumbnail::ThumbnailCategory;
use crate::filesystem;
use crate::model::post::{PostRelation, PostTag};
use crate::resource::post::Note;
use crate::schema::{post, post_note, post_relation, post_tag};
use crate::time::DateTime;
use diesel::prelude::*;
use image::DynamicImage;

/// Updates last_edit_time of post with given `post_id`.
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
    thumbnail: DynamicImage,
    thumbnail_type: ThumbnailCategory,
) -> ApiResult<()> {
    filesystem::delete_post_thumbnail(post_hash, thumbnail_type)?;
    let thumbnail_size = filesystem::save_post_thumbnail(post_hash, thumbnail, thumbnail_type)?;
    match thumbnail_type {
        ThumbnailCategory::Generated => diesel::update(post::table.find(post_hash.id()))
            .set(post::generated_thumbnail_size.eq(thumbnail_size as i64))
            .execute(conn)?,
        ThumbnailCategory::Custom => diesel::update(post::table.find(post_hash.id()))
            .set(post::custom_thumbnail_size.eq(thumbnail_size as i64))
            .execute(conn)?,
    };
    Ok(())
}

/// Creates relations for the post with id `post_id`, symmetrically.
pub fn create_relations(conn: &mut PgConnection, post_id: i64, relations: Vec<i64>) -> QueryResult<()> {
    let new_relations: Vec<_> = relations
        .iter()
        .flat_map(|&other_id| PostRelation::new_pair(post_id, other_id))
        .collect();
    new_relations.insert_into(post_relation::table).execute(conn)?;
    Ok(())
}

/// Deletes all relations involving the post with id `post_id`.
/// Returns number of relations deleted.
pub fn delete_relations(conn: &mut PgConnection, post_id: i64) -> QueryResult<usize> {
    diesel::delete(post_relation::table)
        .filter(post_relation::parent_id.eq(post_id))
        .or_filter(post_relation::child_id.eq(post_id))
        .execute(conn)
}

/// Adds tags to the post with id `post_id`.
pub fn add_tags(conn: &mut PgConnection, post_id: i64, tags: Vec<i64>) -> QueryResult<()> {
    let new_post_tags: Vec<_> = tags.into_iter().map(|tag_id| PostTag { post_id, tag_id }).collect();
    new_post_tags.insert_into(post_tag::table).execute(conn)?;
    Ok(())
}

/// Removes all tags from post with id `post_id`.
/// Returns number of tags removed.
pub fn delete_tags(conn: &mut PgConnection, post_id: i64) -> QueryResult<usize> {
    diesel::delete(post_tag::table)
        .filter(post_tag::post_id.eq(post_id))
        .execute(conn)
}

/// Adds notes to the post with id `post_id`.
pub fn add_notes(conn: &mut PgConnection, post_id: i64, notes: Vec<Note>) -> QueryResult<()> {
    let new_post_notes: Vec<_> = notes.iter().map(|note| note.to_new_post_note(post_id)).collect();
    new_post_notes.insert_into(post_note::table).execute(conn)?;
    Ok(())
}

/// Deletes all notes from post with id `post_id`.
/// Returns number of notes deleted.
pub fn delete_notes(conn: &mut PgConnection, post_id: i64) -> QueryResult<usize> {
    diesel::delete(post_note::table)
        .filter(post_note::post_id.eq(post_id))
        .execute(conn)
}
