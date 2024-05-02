use crate::schema::comment;
use crate::schema::comment_score;
use chrono::DateTime;
use chrono::Utc;
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
    pub fn score(&self, conn: &mut PgConnection) -> QueryResult<i64> {
        comment_score::table
            .filter(comment_score::comment_id.eq(self.id))
            .select(diesel::dsl::sum(comment_score::score))
            .first::<Option<i64>>(conn)
            .map(|n| n.unwrap_or(0))
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

#[cfg(test)]
mod test {
    use super::Comment;
    use crate::schema::comment_score;
    use crate::test::*;
    use chrono::TimeZone;
    use diesel::prelude::*;
    use diesel::result::Error;

    #[test]
    fn test_saving_comment() {
        let mut conn = crate::establish_connection().unwrap_or_else(|err| panic!("{err}"));
        let comment = conn.test_transaction::<Comment, Error, _>(|conn| {
            create_test_user(conn)
                .and_then(|user| create_test_post(conn, user.id))
                .and_then(|post| create_test_comment(conn, post.user_id.unwrap(), post.id))
        });

        let y2k = chrono::Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();
        assert_eq!(comment.text, "text");
        assert_eq!(comment.creation_time, y2k);
    }

    #[test]
    fn test_cascade_deletions() {
        let mut conn = crate::establish_connection().unwrap_or_else(|err| panic!("{err}"));
        conn.test_transaction::<_, Error, _>(|conn| {
            let comment_score_count = comment_score::table.count().first::<i64>(conn)?;
            let comment_score = create_test_user(conn)
                .and_then(|user| create_test_post(conn, user.id))
                .and_then(|post| create_test_comment(conn, post.user_id.unwrap(), post.id))
                .and_then(|comment| create_test_comment_score(conn, comment.user_id, comment.id))?;

            assert_eq!(
                comment_score_count + 1,
                comment_score::table.count().first::<i64>(conn)?
            );

            let num_deleted = diesel::delete(
                comment_score::table.filter(comment_score::user_id.eq(comment_score.user_id)),
            )
            .execute(conn)?;
            assert_eq!(num_deleted, 1);
            assert_eq!(
                comment_score_count,
                comment_score::table.count().first::<i64>(conn)?
            );

            Ok(())
        });
    }
}
