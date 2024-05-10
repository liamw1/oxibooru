use crate::model::pool::{NewPoolCategory, PoolCategory};
use crate::model::post::{NewPost, NewPostNote, NewPostSignature, Post, PostNote, PostSignature};
use crate::model::privilege::UserPrivilege;
use crate::model::user::{NewUser, NewUserToken, User, UserToken};
use crate::schema::{pool_category, post, post_note, post_signature, user, user_token};
use chrono::{DateTime, Utc};
use diesel::prelude::*;

pub const TEST_PRIVILEGE: UserPrivilege = UserPrivilege::Regular;
pub const TEST_USERNAME: &str = "test_user";
pub const TEST_PASSWORD: &str = "test_password";
pub const TEST_SALT: &str = "test_salt";
pub const TEST_HASH: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$Y2hhbmdldGVzdF9zYWx0$zHr+zrjtMKzg7wvAZBGg7YCvLHz04UJ+pZArvplfv0o";

pub fn establish_connection_or_panic() -> PgConnection {
    crate::establish_connection().unwrap_or_else(|err| panic!("{err}"))
}

pub fn create_test_user(conn: &mut PgConnection, name: &str) -> QueryResult<User> {
    let new_user = NewUser {
        name,
        password_hash: TEST_HASH,
        password_salt: TEST_SALT,
        rank: TEST_PRIVILEGE,
    };
    diesel::insert_into(user::table)
        .values(&new_user)
        .returning(User::as_returning())
        .get_result(conn)
}

pub fn create_test_user_token(
    conn: &mut PgConnection,
    user: &User,
    enabled: bool,
    expiration_time: Option<DateTime<Utc>>,
) -> QueryResult<UserToken> {
    let new_user_token = NewUserToken {
        user_id: user.id,
        token: "dummy",
        enabled,
        expiration_time,
    };
    diesel::insert_into(user_token::table)
        .values(&new_user_token)
        .returning(UserToken::as_returning())
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
    };
    diesel::insert_into(post::table)
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
    diesel::insert_into(post_note::table)
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
    diesel::insert_into(post_signature::table)
        .values(&new_post_signature)
        .returning(PostSignature::as_returning())
        .get_result(conn)
}

pub fn create_test_pool_category(conn: &mut PgConnection) -> QueryResult<PoolCategory> {
    let new_pool_category = NewPoolCategory {
        name: "test_pool",
        color: "white",
    };
    diesel::insert_into(pool_category::table)
        .values(&new_pool_category)
        .returning(PoolCategory::as_returning())
        .get_result(conn)
}
