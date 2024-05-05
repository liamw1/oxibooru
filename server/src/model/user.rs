use crate::model::comment::{Comment, CommentScore, NewComment, NewCommentScore};
use crate::model::post::{NewPostFavorite, NewPostFeature, NewPostScore, Post, PostFavorite, PostFeature, PostScore};
use crate::schema::{comment, comment_score, post, post_favorite, post_feature, post_score, user, user_token};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use std::option::Option;

#[derive(Insertable)]
#[diesel(table_name = user)]
pub struct NewUser<'a> {
    pub name: &'a str,
    pub password_hash: &'a str,
    pub rank: &'a str,
    pub creation_time: DateTime<Utc>,
    pub last_login_time: DateTime<Utc>,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = user)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub id: i32,
    pub name: String,
    pub password_hash: String,
    pub password_salt: Option<String>,
    pub email: Option<String>,
    pub rank: String,
    pub creation_time: DateTime<Utc>,
    pub last_login_time: DateTime<Utc>,
}

impl User {
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

#[derive(Queryable, Selectable)]
#[diesel(table_name = user_token)]
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
