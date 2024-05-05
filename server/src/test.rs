use crate::model::pool::{NewPoolCategory, PoolCategory};
use crate::model::post::{NewPost, NewPostNote, NewPostSignature, Post, PostNote, PostSignature};
use crate::model::user::{NewUser, User};
use chrono::{DateTime, TimeZone, Utc};
use diesel::prelude::*;

pub fn test_time() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap()
}

pub fn establish_connection_or_panic() -> PgConnection {
    crate::establish_connection().unwrap_or_else(|err| panic!("{err}"))
}

pub fn create_test_user(conn: &mut PgConnection) -> QueryResult<User> {
    let new_user = NewUser {
        name: "test_user",
        password_hash: "test_password",
        rank: "test",
        creation_time: test_time(),
        last_login_time: test_time(),
    };
    diesel::insert_into(crate::schema::user::table)
        .values(&new_user)
        .returning(User::as_returning())
        .get_result(conn)
}

pub fn create_test_post(conn: &mut PgConnection, user: &User) -> QueryResult<Post> {
    let new_post = NewPost {
        user_id: user.id,
        file_size: 64,
        width: 64,
        height: 64,
        safety: "safe",
        file_type: "image",
        mime_type: "png",
        checksum: "",
        creation_time: test_time(),
        last_edit_time: test_time(),
    };
    diesel::insert_into(crate::schema::post::table)
        .values(&new_post)
        .returning(Post::as_returning())
        .get_result(conn)
}

pub fn create_test_post_note(conn: &mut PgConnection, post: &Post) -> QueryResult<PostNote> {
    let new_post_note = NewPostNote {
        post_id: post.id,
        polygon: &[],
        text: String::from("This is a test note"),
    };
    diesel::insert_into(crate::schema::post_note::table)
        .values(&new_post_note)
        .returning(PostNote::as_returning())
        .get_result(conn)
}

pub fn create_test_post_signature(conn: &mut PgConnection, post: &Post) -> QueryResult<PostSignature> {
    let new_post_signature = NewPostSignature {
        post_id: post.id,
        signature: &[],
        words: &[],
    };
    diesel::insert_into(crate::schema::post_signature::table)
        .values(&new_post_signature)
        .returning(PostSignature::as_returning())
        .get_result(conn)
}

pub fn create_test_pool_category(conn: &mut PgConnection) -> QueryResult<PoolCategory> {
    let new_pool_category = NewPoolCategory {
        name: "test_pool",
        color: "white",
    };
    diesel::insert_into(crate::schema::pool_category::table)
        .values(&new_pool_category)
        .returning(PoolCategory::as_returning())
        .get_result(conn)
}
