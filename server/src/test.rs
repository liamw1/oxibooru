use crate::db;
use crate::model::enums::{AvatarStyle, MimeType, PostFlags, UserRank};
use crate::model::enums::{PostSafety, PostType};
use crate::model::post::{NewPost, Post};
use crate::model::user::{NewUser, NewUserToken, User, UserToken};
use crate::schema::{post, user, user_token};
use crate::time::DateTime;
use diesel::prelude::*;
use diesel::result::Error;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const TEST_PRIVILEGE: UserRank = UserRank::Regular;
pub const TEST_USERNAME: &str = "test_user";
pub const TEST_PASSWORD: &str = "test_password";
pub const TEST_SALT: &str = "test_salt";
pub const TEST_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$dGVzdF9zYWx0$voqGcDZhS6JWiMJy9q12zBgrC6OTBKa9dL8k0O8gD4M";
pub const TEST_TOKEN: Uuid = uuid::uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8");

/// Returns path to a test asset.
pub fn asset_path(relative_path: &Path) -> PathBuf {
    let mut path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    path.push("assets");
    path.push("test");
    path.push(relative_path);
    path
}

/// Used in place of conn.test_transaction as that function doesn't give any useful information on failure
pub fn test_transaction<F, R>(function: F) -> R
where
    F: FnOnce(&mut PgConnection) -> QueryResult<R>,
{
    db::get_connection()
        .unwrap()
        .test_transaction::<_, Error, _>(|conn| Ok(function(conn).unwrap()))
}

/// Inserts a dummy user with username `name` into the database.
/// Returns database [User].
pub fn create_test_user(conn: &mut PgConnection, name: &str) -> QueryResult<User> {
    let new_user = NewUser {
        name,
        password_hash: TEST_HASH,
        password_salt: TEST_SALT,
        email: None,
        rank: TEST_PRIVILEGE,
        avatar_style: AvatarStyle::Manual,
    };
    diesel::insert_into(user::table)
        .values(new_user)
        .returning(User::as_returning())
        .get_result(conn)
}

/// Inserts a dummy user token of user with username `name` into the database.
/// Returns database [UserToken].
pub fn create_test_user_token(
    conn: &mut PgConnection,
    user: &User,
    enabled: bool,
    expiration_time: Option<DateTime>,
) -> QueryResult<UserToken> {
    let new_user_token = NewUserToken {
        user_id: user.id,
        token: TEST_TOKEN,
        note: None,
        enabled,
        expiration_time,
    };
    diesel::insert_into(user_token::table)
        .values(new_user_token)
        .returning(UserToken::as_returning())
        .get_result(conn)
}

/// Inserts a dummy post uploaded by user with username `name` into the database.
/// Returns database [Post].
pub fn create_test_post(conn: &mut PgConnection, user: &User) -> QueryResult<Post> {
    let new_post = NewPost {
        user_id: Some(user.id),
        file_size: 64,
        width: 64,
        height: 64,
        safety: PostSafety::Safe,
        type_: PostType::Image,
        mime_type: MimeType::Png,
        checksum: "",
        checksum_md5: "",
        flags: PostFlags::default(),
        source: None,
    };
    diesel::insert_into(post::table)
        .values(new_post)
        .returning(Post::as_returning())
        .get_result(conn)
}
