use crate::func::auth;
use crate::model::comment::{Comment, CommentScore, NewComment, NewCommentScore};
use crate::model::post::{NewPostFavorite, NewPostFeature, NewPostScore, Post, PostFavorite, PostFeature, PostScore};
use crate::model::privilege::UserPrivilege;
use crate::schema::{comment, comment_score, post, post_favorite, post_feature, post_score, user, user_token};
use crate::util;
use argon2::password_hash::SaltString;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use rand_core::OsRng;
use std::option::Option;
use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum UserCreationError {
    Authentication(#[from] auth::AuthenticationError),
    Insertion(#[from] diesel::result::Error),
}

#[derive(Insertable)]
#[diesel(table_name = user)]
pub struct NewUser<'a> {
    pub name: &'a str,
    pub password_hash: &'a str,
    pub password_salt: &'a str,
    pub rank: UserPrivilege,
    pub creation_time: DateTime<Utc>,
    pub last_login_time: DateTime<Utc>,
}

#[derive(Identifiable, Queryable, Selectable)]
#[diesel(table_name = user)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub id: i32,
    pub name: String,
    pub password_hash: String,
    pub password_salt: String,
    pub email: Option<String>,
    pub rank: UserPrivilege,
    pub creation_time: DateTime<Utc>,
    pub last_login_time: DateTime<Utc>,
}

impl User {
    pub fn new(
        conn: &mut PgConnection,
        name: &str,
        password: &str,
        rank: UserPrivilege,
    ) -> Result<User, UserCreationError> {
        let salt = SaltString::generate(&mut OsRng);
        let hash = auth::hash_password(password, salt.as_str())?;
        let now = Utc::now();

        let new_user = NewUser {
            name,
            password_hash: &hash,
            password_salt: salt.as_str(),
            rank,
            creation_time: now,
            last_login_time: now,
        };

        diesel::insert_into(user::table)
            .values(&new_user)
            .returning(User::as_returning())
            .get_result(conn)
            .map_err(|err| UserCreationError::from(err))
    }

    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        user::table.count().first(conn)
    }

    pub fn post_count(&self, conn: &mut PgConnection) -> QueryResult<i64> {
        post::table.filter(post::user_id.eq(self.id)).count().first(conn)
    }

    pub fn comment_count(&self, conn: &mut PgConnection) -> QueryResult<i64> {
        comment::table.filter(comment::user_id.eq(self.id)).count().first(conn)
    }

    pub fn favorite_post_count(&self, conn: &mut PgConnection) -> QueryResult<i64> {
        post_favorite::table
            .filter(post_favorite::user_id.eq(self.id))
            .count()
            .first(conn)
    }

    pub fn liked_post_count(&self, conn: &mut PgConnection) -> QueryResult<i64> {
        post_score::table
            .filter(post_score::user_id.eq(self.id))
            .filter(post_score::score.eq(1))
            .count()
            .first(conn)
    }

    pub fn disliked_post_count(&self, conn: &mut PgConnection) -> QueryResult<i64> {
        post_score::table
            .filter(post_score::user_id.eq(self.id))
            .filter(post_score::score.eq(-1))
            .count()
            .first(conn)
    }

    pub fn add_comment(&self, conn: &mut PgConnection, post: &Post, text: &str) -> QueryResult<Comment> {
        let now = Utc::now();

        let new_comment = NewComment {
            user_id: self.id,
            post_id: post.id,
            text,
            creation_time: now,
            last_edit_time: now,
        };

        diesel::insert_into(comment::table)
            .values(&new_comment)
            .returning(Comment::as_returning())
            .get_result(conn)
    }

    pub fn like_comment(&self, conn: &mut PgConnection, comment: &Comment) -> QueryResult<CommentScore> {
        let new_comment_score = NewCommentScore {
            comment_id: comment.id,
            user_id: self.id,
            score: 1,
            time: chrono::Utc::now(),
        };

        diesel::insert_into(comment_score::table)
            .values(&new_comment_score)
            .returning(CommentScore::as_returning())
            .get_result(conn)
    }

    pub fn dislike_comment(&self, conn: &mut PgConnection, comment: &Comment) -> QueryResult<CommentScore> {
        let new_comment_score = NewCommentScore {
            comment_id: comment.id,
            user_id: self.id,
            score: -1,
            time: chrono::Utc::now(),
        };

        diesel::insert_into(comment_score::table)
            .values(&new_comment_score)
            .returning(CommentScore::as_returning())
            .get_result(conn)
    }

    pub fn like_post(&self, conn: &mut PgConnection, post: &Post) -> QueryResult<PostScore> {
        let new_post_score = NewPostScore {
            post_id: post.id,
            user_id: self.id,
            score: 1,
            time: chrono::Utc::now(),
        };

        diesel::insert_into(post_score::table)
            .values(&new_post_score)
            .returning(PostScore::as_returning())
            .get_result(conn)
    }

    pub fn favorite_post(&self, conn: &mut PgConnection, post: &Post) -> QueryResult<PostFavorite> {
        let new_post_favorite = NewPostFavorite {
            post_id: post.id,
            user_id: self.id,
            time: Utc::now(),
        };

        diesel::insert_into(post_favorite::table)
            .values(&new_post_favorite)
            .returning(PostFavorite::as_returning())
            .get_result(conn)
    }

    pub fn feature_post(&self, conn: &mut PgConnection, post: &Post) -> QueryResult<PostFeature> {
        let new_post_feature = NewPostFeature {
            post_id: post.id,
            user_id: self.id,
        };

        diesel::insert_into(post_feature::table)
            .values(&new_post_feature)
            .returning(PostFeature::as_returning())
            .get_result(conn)
    }

    pub fn delete(self, conn: &mut PgConnection) -> QueryResult<()> {
        conn.transaction(|conn| util::validate_uniqueness("user", diesel::delete(&self).execute(conn)?))
    }
}

#[derive(Insertable)]
#[diesel(table_name = user_token)]
pub struct NewUserToken<'a> {
    pub user_id: i32,
    pub token: &'a str,
    pub enabled: bool,
    pub expiration_time: Option<DateTime<Utc>>,
    pub creation_time: DateTime<Utc>,
    pub last_edit_time: DateTime<Utc>,
    pub last_usage_time: DateTime<Utc>,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(User))]
#[diesel(table_name = user_token)]
#[diesel(primary_key(user_id))]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UserToken {
    pub user_id: i32,
    pub token: String,
    pub note: Option<String>,
    pub enabled: bool,
    pub expiration_time: Option<DateTime<Utc>>,
    pub creation_time: DateTime<Utc>,
    pub last_edit_time: DateTime<Utc>,
    pub last_usage_time: DateTime<Utc>,
}

impl UserToken {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        user_token::table.count().first(conn)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::model::comment::{Comment, CommentScore};
    use crate::model::post::{Post, PostFavorite, PostFeature, PostScore};
    use crate::test::*;
    use diesel::result::Error;

    #[test]
    fn test_saving_user() {
        let user = establish_connection_or_panic().test_transaction(|conn| create_test_user(conn, TEST_USERNAME));

        assert_eq!(user.name, TEST_USERNAME, "Incorrect user name");
        assert_eq!(user.password_hash, TEST_HASH, "Incorrect user password hash");
        assert_eq!(user.password_salt, TEST_SALT, "Incorrect user password salt");
        assert_eq!(user.rank, TEST_PRIVILEGE, "Incorrect user rank");
        assert_eq!(user.creation_time, test_time(), "Incorrect user creation time");
    }

    #[test]
    fn test_saving_user_token() {
        let user_token = establish_connection_or_panic().test_transaction(|conn| {
            create_test_user(conn, "test_user").and_then(|user| create_test_user_token(conn, &user, false, None))
        });

        assert!(!user_token.enabled, "Test user token should not be enabled");
        assert_eq!(user_token.expiration_time, None, "Incorrect user token expiration time");
        assert_eq!(user_token.creation_time, test_time(), "Incorrect user token creation time");
    }

    #[test]
    fn test_user_statistics() {
        establish_connection_or_panic().test_transaction::<_, Error, _>(|conn| {
            let user = create_test_user(conn, "test_user")?;

            assert_eq!(user.post_count(conn)?, 0, "User should have no posts");
            assert_eq!(user.comment_count(conn)?, 0, "User should have no comments");
            assert_eq!(user.liked_post_count(conn)?, 0, "User should have no liked posts");
            assert_eq!(user.favorite_post_count(conn)?, 0, "User should have no favorite posts");

            let post = create_test_post(conn, &user)?;
            user.add_comment(conn, &post, "test comment")?;
            user.like_post(conn, &post)?;
            user.favorite_post(conn, &post)?;

            assert_eq!(user.post_count(conn)?, 1, "User should have one post");
            assert_eq!(user.comment_count(conn)?, 1, "User should have one comment");
            assert_eq!(user.liked_post_count(conn)?, 1, "User should have one liked post");
            assert_eq!(user.favorite_post_count(conn)?, 1, "User should have one favorite post");

            Ok(())
        })
    }

    #[test]
    fn test_cascade_deletions() {
        establish_connection_or_panic().test_transaction::<_, Error, _>(|conn| {
            let user_count = User::count(conn)?;
            let post_count = Post::count(conn)?;
            let post_score_count = PostScore::count(conn)?;
            let post_favorite_count = PostFavorite::count(conn)?;
            let post_feature_count = PostFeature::count(conn)?;
            let comment_count = Comment::count(conn)?;
            let comment_score_count = CommentScore::count(conn)?;

            let user = create_test_user(conn, "test_user")?;
            let post = create_test_post(conn, &user)?;
            let comment = user.add_comment(conn, &post, "test comment")?;

            user.like_post(conn, &post)?;
            user.favorite_post(conn, &post)?;
            user.feature_post(conn, &post)?;
            user.like_comment(conn, &comment)?;

            assert_eq!(User::count(conn)?, user_count + 1, "User insertion failed");
            assert_eq!(Post::count(conn)?, post_count + 1, "Post insertion failed");
            assert_eq!(PostScore::count(conn)?, post_score_count + 1, "Post score insertion failed");
            assert_eq!(PostFavorite::count(conn)?, post_favorite_count + 1, "Post favorite insertion failed");
            assert_eq!(PostFeature::count(conn)?, post_feature_count + 1, "Post feature insertion failed");
            assert_eq!(Comment::count(conn)?, comment_count + 1, "Comment insertion failed");
            assert_eq!(CommentScore::count(conn)?, comment_score_count + 1, "Comment score insertion failed");

            user.delete(conn)?;

            assert_eq!(User::count(conn)?, user_count, "User deletion failed");
            assert_eq!(Post::count(conn)?, post_count + 1, "Post should not have been deleted");
            assert_eq!(PostScore::count(conn)?, post_score_count, "Post score cascade deletion failed");
            assert_eq!(PostFavorite::count(conn)?, post_favorite_count, "Post favorite cascade deletion failed");
            assert_eq!(PostFeature::count(conn)?, post_feature_count, "Post feature cascade deletion failed");
            assert_eq!(Comment::count(conn)?, comment_count, "Comment cascade deletion failed");
            assert_eq!(CommentScore::count(conn)?, comment_score_count, "Comment score cascade deletion failed");

            Ok(())
        })
    }
}
