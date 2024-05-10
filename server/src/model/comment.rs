use crate::model::post::Post;
use crate::model::user::User;
use crate::schema::{comment, comment_score};
use crate::util;
use chrono::{DateTime, Utc};
use diesel::prelude::*;

#[derive(Insertable)]
#[diesel(table_name = comment)]
pub struct NewComment<'a> {
    pub user_id: i32,
    pub post_id: i32,
    pub text: &'a str,
    pub creation_time: DateTime<Utc>,
    pub last_edit_time: DateTime<Utc>,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(User), belongs_to(Post))]
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
        CommentScore::belonging_to(self)
            .select(diesel::dsl::sum(comment_score::score))
            .first::<Option<i64>>(conn)
            .map(|n| n.unwrap_or(0))
    }

    pub fn delete(self, conn: &mut PgConnection) -> QueryResult<()> {
        conn.transaction(|conn| util::validate_uniqueness("comment", diesel::delete(&self).execute(conn)?))
    }
}

pub type NewCommentScore = CommentScore;

#[derive(Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Comment), belongs_to(User))]
#[diesel(table_name = comment_score)]
#[diesel(primary_key(comment_id, user_id))]
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
    use super::*;
    use crate::model::user::User;
    use crate::test::*;
    use diesel::result::Error;

    #[test]
    fn test_saving_comment() {
        let comment_text = "This is a test comment";
        let comment = establish_connection_or_panic().test_transaction(|conn| {
            let user = create_test_user(conn, TEST_USERNAME)?;
            create_test_post(conn, &user).and_then(|post| user.add_comment(conn, &post, comment_text))
        });

        assert_eq!(comment.text, comment_text, "Incorrect comment text");
    }

    #[test]
    fn test_cascade_deletions() {
        establish_connection_or_panic().test_transaction::<_, Error, _>(|conn| {
            let user_count = User::count(conn)?;
            let comment_count = Comment::count(conn)?;
            let comment_score_count = CommentScore::count(conn)?;

            let user = create_test_user(conn, TEST_USERNAME)?;
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

    #[test]
    fn test_comment_scoring() {
        establish_connection_or_panic().test_transaction::<_, Error, _>(|conn| {
            let user1 = create_test_user(conn, "user1")?;
            let user2 = create_test_user(conn, "user2")?;
            let user3 = create_test_user(conn, "user3")?;
            let user4 = create_test_user(conn, "user4")?;
            let user5 = create_test_user(conn, "user5")?;
            let comment = create_test_post(conn, &user1)
                .and_then(|post| user1.add_comment(conn, &post, "This is a test comment"))?;
            user1.like_comment(conn, &comment)?;

            assert_eq!(comment.score(conn)?, 1, "Comment should have a score of 1");

            user2.like_comment(conn, &comment)?;

            assert_eq!(comment.score(conn)?, 2, "Comment should have a score of 2");

            user3.dislike_comment(conn, &comment)?;
            user4.dislike_comment(conn, &comment)?;
            user5.dislike_comment(conn, &comment)?;

            assert_eq!(comment.score(conn)?, -1, "Comment should have score of -1");

            Ok(())
        });
    }
}
