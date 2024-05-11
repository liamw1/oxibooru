use crate::model::comment::Comment;
use crate::schema::comment;
use diesel::prelude::*;

pub fn get_comment(conn: &mut PgConnection, comment_id: i32) -> QueryResult<Comment> {
    comment::table.find(comment_id).select(Comment::as_select()).first(conn)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test::*;

    #[test]
    fn test_get_comment() {
        test_transaction(|conn: &mut PgConnection| {
            let user = create_test_user(conn, "test_user")?;
            let comment =
                create_test_post(conn, &user).and_then(|post| user.add_comment(conn, &post, "test_comment"))?;

            assert_eq!(get_comment(conn, comment.id)?, comment);
            assert!(get_comment(conn, comment.id + 1).is_err());

            Ok(())
        })
    }
}
