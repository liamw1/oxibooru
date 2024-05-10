use crate::model::comment::Comment;
use crate::schema::comment;
use diesel::prelude::*;

pub fn get_comment(conn: &mut PgConnection, comment_id: i32) -> QueryResult<Comment> {
    comment::table.find(comment_id).select(Comment::as_select()).first(conn)
}
