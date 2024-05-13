use crate::func::auth;
use crate::model::comment::{Comment, CommentScore, NewComment, NewCommentScore};
use crate::model::post::{NewPostFavorite, NewPostFeature, NewPostScore, Post, PostFavorite, PostFeature, PostScore};
use crate::model::privilege::UserPrivilege;
use crate::model::TableName;
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
}

#[derive(Debug, PartialEq, Eq, Identifiable, Queryable, Selectable)]
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

impl TableName for User {
    fn table_name() -> &'static str {
        "user"
    }
}

impl User {
    pub fn new(
        conn: &mut PgConnection,
        name: &str,
        password: &str,
        rank: UserPrivilege,
    ) -> Result<Self, UserCreationError> {
        let salt = SaltString::generate(&mut OsRng);
        let hash = auth::hash_password(password, salt.as_str())?;
        let new_user = NewUser {
            name,
            password_hash: &hash,
            password_salt: salt.as_str(),
            rank,
        };
        diesel::insert_into(user::table)
            .values(&new_user)
            .returning(Self::as_returning())
            .get_result(conn)
            .map_err(UserCreationError::from)
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
        let new_comment = NewComment {
            user_id: self.id,
            post_id: post.id,
            text,
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
        util::delete(conn, &self)
    }
}

#[derive(Insertable)]
#[diesel(table_name = user_token)]
pub struct NewUserToken<'a> {
    pub user_id: i32,
    pub token: &'a str,
    pub enabled: bool,
    pub expiration_time: Option<DateTime<Utc>>,
}

#[derive(Debug, PartialEq, Eq, Associations, Identifiable, Queryable, Selectable)]
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

    #[test]
    fn test_saving_user() {
        let user = test_transaction(|conn: &mut PgConnection| create_test_user(conn, TEST_USERNAME));

        assert_eq!(user.name, TEST_USERNAME);
        assert_eq!(user.password_hash, TEST_HASH);
        assert_eq!(user.password_salt, TEST_SALT);
        assert_eq!(user.rank, TEST_PRIVILEGE);
    }

    #[test]
    fn test_saving_user_token() {
        let user_token = test_transaction(|conn: &mut PgConnection| {
            create_test_user(conn, "test_user").and_then(|user| create_test_user_token(conn, &user, false, None))
        });

        assert!(!user_token.enabled);
        assert_eq!(user_token.expiration_time, None);
    }

    #[test]
    fn test_user_statistics() {
        test_transaction(|conn: &mut PgConnection| {
            let user = create_test_user(conn, "test_user")?;

            assert_eq!(user.post_count(conn)?, 0);
            assert_eq!(user.comment_count(conn)?, 0);
            assert_eq!(user.liked_post_count(conn)?, 0);
            assert_eq!(user.favorite_post_count(conn)?, 0);

            let post = create_test_post(conn, &user)?;
            user.add_comment(conn, &post, "test comment")?;
            user.like_post(conn, &post)?;
            user.favorite_post(conn, &post)?;

            assert_eq!(user.post_count(conn)?, 1);
            assert_eq!(user.comment_count(conn)?, 1);
            assert_eq!(user.liked_post_count(conn)?, 1);
            assert_eq!(user.favorite_post_count(conn)?, 1);

            Ok(())
        });
    }

    #[test]
    fn test_cascade_deletions() {
        test_transaction(|conn: &mut PgConnection| {
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

            assert_eq!(User::count(conn)?, user_count + 1);
            assert_eq!(Post::count(conn)?, post_count + 1);
            assert_eq!(PostScore::count(conn)?, post_score_count + 1);
            assert_eq!(PostFavorite::count(conn)?, post_favorite_count + 1);
            assert_eq!(PostFeature::count(conn)?, post_feature_count + 1);
            assert_eq!(Comment::count(conn)?, comment_count + 1);
            assert_eq!(CommentScore::count(conn)?, comment_score_count + 1);

            user.delete(conn)?;

            assert_eq!(User::count(conn)?, user_count);
            assert_eq!(Post::count(conn)?, post_count + 1);
            assert_eq!(PostScore::count(conn)?, post_score_count);
            assert_eq!(PostFavorite::count(conn)?, post_favorite_count);
            assert_eq!(PostFeature::count(conn)?, post_feature_count);
            assert_eq!(Comment::count(conn)?, comment_count);
            assert_eq!(CommentScore::count(conn)?, comment_score_count);

            Ok(())
        });
    }
}
