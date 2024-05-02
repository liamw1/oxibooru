use crate::model::comment::{Comment, CommentScore, NewComment, NewCommentScore};
use crate::model::post::{NewPost, Post};
use crate::model::user::{NewUser, User};
use chrono::TimeZone;
use diesel::prelude::*;

pub fn create_test_user(conn: &mut PgConnection) -> QueryResult<User> {
    let y2k = chrono::Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();

    let new_user = NewUser {
        name: "test-user",
        password_hash: "test-password",
        rank: "test",
        creation_time: y2k,
        last_login_time: y2k,
    };

    diesel::insert_into(crate::schema::user::table)
        .values(&new_user)
        .returning(User::as_returning())
        .get_result(conn)
}

pub fn create_test_post(conn: &mut PgConnection, user_id: i32) -> QueryResult<Post> {
    let y2k = chrono::Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();

    let new_post = NewPost {
        user_id,
        file_size: 64,
        width: 64,
        height: 64,
        safety: "safe",
        file_type: "image",
        mime_type: "png",
        checksum: "",
        creation_time: y2k,
        last_edit_time: y2k,
    };

    diesel::insert_into(crate::schema::post::table)
        .values(&new_post)
        .returning(Post::as_returning())
        .get_result(conn)
}

pub fn create_test_comment(
    conn: &mut PgConnection,
    user_id: i32,
    post_id: i32,
) -> QueryResult<Comment> {
    let y2k = chrono::Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();

    let new_comment = NewComment {
        user_id,
        post_id,
        text: "text",
        creation_time: y2k,
        last_edit_time: y2k,
    };

    diesel::insert_into(crate::schema::comment::table)
        .values(&new_comment)
        .returning(Comment::as_returning())
        .get_result(conn)
}

pub fn create_test_comment_score(
    conn: &mut PgConnection,
    user_id: i32,
    comment_id: i32,
) -> QueryResult<CommentScore> {
    let y2k = chrono::Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();

    let new_comment_score = NewCommentScore {
        comment_id,
        user_id,
        score: 1,
        time: y2k,
    };

    diesel::insert_into(crate::schema::comment_score::table)
        .values(&new_comment_score)
        .returning(CommentScore::as_returning())
        .get_result(conn)
}
