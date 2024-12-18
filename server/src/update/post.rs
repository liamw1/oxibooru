use crate::model::post::{PostRelation, PostTag};
use crate::resource::post::Note;
use crate::schema::{post_note, post_relation, post_tag};
use diesel::prelude::*;

/// Creates relations for the post with id `post_id`, symmetrically.
pub fn create_relations(conn: &mut PgConnection, post_id: i32, relations: Vec<i32>) -> QueryResult<()> {
    let post_as_parent = relations.iter().map(|&child_id| PostRelation {
        parent_id: post_id,
        child_id,
    });
    let post_as_child = relations.iter().map(|&parent_id| PostRelation {
        parent_id,
        child_id: post_id,
    });
    let updated_relations: Vec<_> = post_as_parent.chain(post_as_child).collect();
    diesel::insert_into(post_relation::table)
        .values(updated_relations)
        .execute(conn)?;
    Ok(())
}

/// Deletes all relations involving the post with id `post_id`.
/// Returns number of relations deleted.
pub fn delete_relations(conn: &mut PgConnection, post_id: i32) -> QueryResult<usize> {
    diesel::delete(post_relation::table)
        .filter(post_relation::parent_id.eq(post_id))
        .or_filter(post_relation::child_id.eq(post_id))
        .execute(conn)
}

/// Adds tags to the post with id `post_id`.
pub fn add_tags(conn: &mut PgConnection, post_id: i32, tags: Vec<i32>) -> QueryResult<()> {
    let new_post_tags: Vec<_> = tags.into_iter().map(|tag_id| PostTag { post_id, tag_id }).collect();
    diesel::insert_into(post_tag::table)
        .values(new_post_tags)
        .execute(conn)?;
    Ok(())
}

/// Removes all tags from post with id `post_id`.
/// Returns number of tags removed.
pub fn delete_tags(conn: &mut PgConnection, post_id: i32) -> QueryResult<usize> {
    diesel::delete(post_tag::table)
        .filter(post_tag::post_id.eq(post_id))
        .execute(conn)
}

/// Adds notes to the post with id `post_id`.
pub fn add_notes(conn: &mut PgConnection, post_id: i32, notes: Vec<Note>) -> QueryResult<()> {
    let new_post_notes: Vec<_> = notes.iter().map(|note| note.to_new_post_note(post_id)).collect();
    diesel::insert_into(post_note::table)
        .values(new_post_notes)
        .execute(conn)?;
    Ok(())
}

/// Deletes all notes from post with id `post_id`.
/// Returns number of notes deleted.
pub fn delete_notes(conn: &mut PgConnection, post_id: i32) -> QueryResult<usize> {
    diesel::delete(post_note::table)
        .filter(post_note::post_id.eq(post_id))
        .execute(conn)
}
