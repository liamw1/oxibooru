use crate::schema::{comment, comment_score};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error};

#[derive(Insertable)]
#[diesel(table_name = comment)]
pub struct NewComment<'a> {
    pub user_id: i32,
    pub post_id: i32,
    pub text: &'a str,
    pub creation_time: DateTime<Utc>,
    pub last_edit_time: DateTime<Utc>,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = comment)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Comment {
    pub id: i32,
    pub user_id: i32,
    pub post_id: i32,
    pub text: String,
    pub creation_time: DateTime<Utc>,
    pub last_edit_time: DateTime<Utc>,
}

impl Comment {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        comment::table.count().first(conn)
    }

    pub fn score(&self, conn: &mut PgConnection) -> QueryResult<i64> {
        comment_score::table
            .filter(comment_score::comment_id.eq(self.id))
            .select(diesel::dsl::sum(comment_score::score))
            .first::<Option<i64>>(conn)
            .map(|n| n.unwrap_or(0))
    }

    pub fn delete(self, conn: &mut PgConnection) -> QueryResult<()> {
        conn.transaction(|conn| {
            let num_deleted = diesel::delete(comment::table.filter(comment::columns::id.eq(self.id))).execute(conn)?;
            let error_message =
                |msg: String| -> Error { Error::DatabaseError(DatabaseErrorKind::UniqueViolation, Box::new(msg)) };
            match num_deleted {
                0 => Err(error_message(format!("Failed to delete comment: no comment with id {}", self.id))),
                1 => Ok(()),
                _ => Err(error_message(format!("Failed to delete comment: id {} is not unique", self.id))),
            }
        })
    }
}

pub type NewCommentScore = CommentScore;

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = comment_score)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct CommentScore {
    pub comment_id: i32,
    pub user_id: i32,
    pub score: i32,
    pub time: DateTime<Utc>,
}

impl CommentScore {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        comment_score::table.count().first(conn)
    }
}

#[cfg(test)]
mod test {
    use super::{Comment, CommentScore};
    use crate::model::user::User;
    use crate::test::*;
    use diesel::prelude::*;
    use diesel::result::Error;

    #[test]
    fn test_saving_comment() {
        let comment_text = "This is a test comment";
        let comment = establish_connection_or_panic().test_transaction::<Comment, Error, _>(|conn| {
            let user = create_test_user(conn)?;
            create_test_post(conn, &user).and_then(|post| user.add_comment(conn, &post, comment_text))
        });

        assert_eq!(comment.text, comment_text, "Comment text does not match");
    }

    #[test]
    fn test_cascade_deletions() {
        establish_connection_or_panic().test_transaction::<_, Error, _>(|conn| {
            let user_count = User::count(conn)?;
            let comment_count = Comment::count(conn)?;
            let comment_score_count = CommentScore::count(conn)?;

            let user = create_test_user(conn)?;
            let comment = create_test_post(conn, &user)
                .and_then(|post| user.add_comment(conn, &post, "This is a test comment"))?;
            user.like_comment(conn, &comment)?;

            assert_eq!(User::count(conn)?, user_count + 1, "User insertion failed");
            assert_eq!(Comment::count(conn)?, comment_count + 1, "Comment insertion failed");
            assert_eq!(CommentScore::count(conn)?, comment_score_count + 1, "Comment score insertion failed");

            comment.delete(conn)?;

            assert_eq!(User::count(conn)?, user_count + 1, "User should not have been deleted");
            assert_eq!(Comment::count(conn)?, comment_count, "Comment deletion failed");
            assert_eq!(CommentScore::count(conn)?, comment_score_count, "Comment score cascade deletion failed");

            Ok(())
        });
    }
}
