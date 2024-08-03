use crate::model::post::{PostRelation, PostTag};
use crate::schema::{post_relation, post_tag};
use diesel::prelude::*;

/*
    Creates relations for the given post symmetrically.
    Does not check for privileges.
*/
pub fn create_relations(conn: &mut PgConnection, post_id: i32, relations: Vec<i32>) -> QueryResult<usize> {
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
        .execute(conn)
}

pub fn delete_relations(conn: &mut PgConnection, post_id: i32) -> QueryResult<usize> {
    diesel::delete(post_relation::table)
        .filter(post_relation::parent_id.eq(post_id))
        .or_filter(post_relation::child_id.eq(post_id))
        .execute(conn)
}

pub fn add_tags(conn: &mut PgConnection, post_id: i32, tags: Vec<i32>) -> QueryResult<()> {
    let new_post_tags: Vec<_> = tags.into_iter().map(|tag_id| PostTag { post_id, tag_id }).collect();
    diesel::insert_into(post_tag::table)
        .values(new_post_tags)
        .execute(conn)?;
    Ok(())
}

pub fn delete_tags(conn: &mut PgConnection, post_id: i32) -> QueryResult<usize> {
    diesel::delete(post_tag::table)
        .filter(post_tag::post_id.eq(post_id))
        .execute(conn)
}
