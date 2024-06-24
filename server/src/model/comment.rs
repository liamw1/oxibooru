use crate::model::post::Post;
use crate::model::user::User;
use crate::model::TableName;
use crate::schema::{comment, comment_score};
use crate::util;
use crate::util::DateTime;
use diesel::prelude::*;

#[derive(Insertable)]
#[diesel(table_name = comment)]
pub struct NewComment<'a> {
    pub user_id: i32,
    pub post_id: i32,
    pub text: &'a str,
}

#[derive(Debug, PartialEq, Eq, Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(User), belongs_to(Post))]
#[diesel(table_name = comment)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Comment {
    pub id: i32,
    pub user_id: i32,
    pub post_id: i32,
    pub text: String,
    pub creation_time: DateTime,
    pub last_edit_time: DateTime,
}

impl TableName for Comment {
    fn table_name() -> &'static str {
        "comment"
    }
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

    pub fn update_text(mut self, conn: &mut PgConnection, text: String) -> QueryResult<Self> {
        self.text = text;
        util::update_single_row(conn, &self, comment::text.eq(&self.text))?;
        Ok(self)
    }

    pub fn delete(self, conn: &mut PgConnection) -> QueryResult<()> {
        util::delete(conn, &self)
    }
}

pub type NewCommentScore = CommentScore;

#[derive(Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Comment), belongs_to(User))]
#[diesel(table_name = comment_score)]
#[diesel(primary_key(comment_id, user_id))]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct CommentScore {
    pub comment_id: i32,
    pub user_id: i32,
    pub score: i32,
    pub time: DateTime,
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

    #[test]
    fn save_comment() {
        let comment_text = "This is a test comment";
        let comment = test_transaction(|conn: &mut PgConnection| {
            let user = create_test_user(conn, TEST_USERNAME)?;
            create_test_post(conn, &user).and_then(|post| user.add_comment(conn, &post, comment_text))
        });

        assert_eq!(comment.text, comment_text);
    }

    #[test]
    fn cascade_deletions() {
        test_transaction(|conn: &mut PgConnection| {
            let user_count = User::count(conn)?;
            let comment_count = Comment::count(conn)?;
            let comment_score_count = CommentScore::count(conn)?;

            let user = create_test_user(conn, TEST_USERNAME)?;
            let comment = create_test_post(conn, &user)
                .and_then(|post| user.add_comment(conn, &post, "This is a test comment"))?;
            user.like_comment(conn, &comment)?;

            assert_eq!(User::count(conn)?, user_count + 1);
            assert_eq!(Comment::count(conn)?, comment_count + 1);
            assert_eq!(CommentScore::count(conn)?, comment_score_count + 1);

            comment.delete(conn)?;

            assert_eq!(User::count(conn)?, user_count + 1);
            assert_eq!(Comment::count(conn)?, comment_count);
            assert_eq!(CommentScore::count(conn)?, comment_score_count);

            Ok(())
        });
    }

    #[test]
    fn comment_scoring() {
        test_transaction(|conn: &mut PgConnection| {
            let user1 = create_test_user(conn, "user1")?;
            let user2 = create_test_user(conn, "user2")?;
            let user3 = create_test_user(conn, "user3")?;
            let user4 = create_test_user(conn, "user4")?;
            let user5 = create_test_user(conn, "user5")?;
            let comment = create_test_post(conn, &user1)
                .and_then(|post| user1.add_comment(conn, &post, "This is a test comment"))?;
            user1.like_comment(conn, &comment)?;

            assert_eq!(comment.score(conn)?, 1);

            user2.like_comment(conn, &comment)?;

            assert_eq!(comment.score(conn)?, 2);

            user3.dislike_comment(conn, &comment)?;
            user4.dislike_comment(conn, &comment)?;
            user5.dislike_comment(conn, &comment)?;

            assert_eq!(comment.score(conn)?, -1);

            Ok(())
        });
    }
}
