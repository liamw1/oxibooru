use crate::schema::comment;
use crate::schema::post;
use crate::schema::post_favorite;
use crate::schema::post_score;
use crate::schema::user;
use crate::schema::user_token;
use chrono::DateTime;
use chrono::Utc;
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
    pub fn post_count(&self, conn: &mut PgConnection) -> QueryResult<i64> {
        post::table
            .filter(post::user_id.eq(self.id))
            .count()
            .first::<i64>(conn)
    }

    pub fn comment_count(&self, conn: &mut PgConnection) -> QueryResult<i64> {
        comment::table
            .filter(comment::user_id.eq(self.id))
            .count()
            .first::<i64>(conn)
    }

    pub fn favorite_post_count(&self, conn: &mut PgConnection) -> QueryResult<i64> {
        post_favorite::table
            .filter(post_favorite::user_id.eq(self.id))
            .count()
            .first::<i64>(conn)
    }

    pub fn liked_post_count(&self, conn: &mut PgConnection) -> QueryResult<i64> {
        post_score::table
            .filter(post_score::user_id.eq(self.id))
            .filter(post_score::score.eq(1))
            .count()
            .first::<i64>(conn)
    }

    pub fn disliked_post_count(&self, conn: &mut PgConnection) -> QueryResult<i64> {
        post_score::table
            .filter(post_score::user_id.eq(self.id))
            .filter(post_score::score.eq(-1))
            .count()
            .first::<i64>(conn)
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
