use crate::model::post::NewPostRelation;
use crate::schema::post_relation;
use diesel::prelude::*;

/*
    Creates relations for the given post symmetrically.
    Does not check for privileges.
*/
pub fn create_relations(conn: &mut PgConnection, post_id: i32, relations: &[i32]) -> QueryResult<usize> {
    let post_as_parent = relations.iter().map(|&child_id| NewPostRelation {
        parent_id: post_id,
        child_id,
    });
    let post_as_child = relations.iter().map(|&parent_id| NewPostRelation {
        parent_id,
        child_id: post_id,
    });
    let updated_relations: Vec<_> = post_as_parent.chain(post_as_child).collect();
    diesel::insert_into(post_relation::table)
        .values(updated_relations)
        .execute(conn)
}
