use crate::model::pool::Pool;
use crate::schema::{
    pool, pool_post, post, post_favorite, post_feature, post_note, post_relation, post_score, post_signature, post_tag,
};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error};
use std::option::Option;

#[derive(Insertable)]
#[diesel(table_name = post)]
pub struct NewPost<'a> {
    pub user_id: i32,
    pub file_size: i64,
    pub width: i32,
    pub height: i32,
    pub safety: &'a str,
    pub file_type: &'a str,
    pub mime_type: &'a str,
    pub checksum: &'a str,
    pub creation_time: DateTime<Utc>,
    pub last_edit_time: DateTime<Utc>,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = post)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Post {
    pub id: i32,
    pub user_id: Option<i32>,
    pub file_size: i64,
    pub width: i32,
    pub height: i32,
    pub safety: String,
    pub file_type: String,
    pub mime_type: String,
    pub checksum: String,
    pub checksum_md5: Option<String>,
    pub flags: Option<String>,
    pub source: Option<String>,
    pub creation_time: DateTime<Utc>,
    pub last_edit_time: DateTime<Utc>,
}

impl Post {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        post::table.count().first(conn)
    }

    pub fn pools_in(&self, conn: &mut PgConnection) -> QueryResult<Vec<Pool>> {
        let pool_ids = pool_post::table
            .filter(pool_post::columns::post_id.eq(self.id))
            .select(pool_post::columns::pool_id);
        pool::table.filter(pool::columns::id.eq_any(pool_ids)).load(conn)
    }

    pub fn delete(self, conn: &mut PgConnection) -> QueryResult<()> {
        conn.transaction(|conn| {
            let num_deleted = diesel::delete(post::table.filter(post::columns::id.eq(self.id))).execute(conn)?;
            let error_message =
                |msg: String| -> Error { Error::DatabaseError(DatabaseErrorKind::UniqueViolation, Box::new(msg)) };
            match num_deleted {
                0 => Err(error_message(format!("Failed to delete post: no post with id {}", self.id))),
                1 => Ok(()),
                _ => Err(error_message(format!("Failed to delete post: id {} is not unique", self.id))),
            }
        })
    }
}

pub type NewPostRelation = PostRelation;

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = post_relation)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PostRelation {
    pub parent_id: i32,
    pub child_id: i32,
}

impl PostRelation {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        post_relation::table.count().first(conn)
    }
}

pub type NewPostTag = PostTag;

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = post_tag)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PostTag {
    pub post_id: i32,
    pub tag_id: i32,
}

impl PostTag {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        post_tag::table.count().first(conn)
    }
}

pub type NewPostFavorite = PostFavorite;

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = post_favorite)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PostFavorite {
    pub post_id: i32,
    pub user_id: i32,
    pub time: DateTime<Utc>,
}

impl PostFavorite {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        post_favorite::table.count().first(conn)
    }
}

#[derive(Insertable)]
#[diesel(table_name = post_feature)]
pub struct NewPostFeature {
    pub post_id: i32,
    pub user_id: i32,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = post_feature)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PostFeature {
    pub id: i32,
    pub post_id: i32,
    pub user_id: i32,
}

impl PostFeature {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        post_feature::table.count().first(conn)
    }
}

#[derive(Insertable)]
#[diesel(table_name = post_note)]
pub struct NewPostNote {
    pub post_id: i32,
    pub polygon: Vec<u8>,
    pub text: String,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = post_note)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PostNote {
    pub id: i32,
    pub post_id: i32,
    pub polygon: Vec<u8>,
    pub text: String,
}

impl PostNote {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        post_note::table.count().first(conn)
    }
}

pub type NewPostScore = PostScore;

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = post_score)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PostScore {
    pub post_id: i32,
    pub user_id: i32,
    pub score: i32,
    pub time: DateTime<Utc>,
}

impl PostScore {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        post_score::table.count().first(conn)
    }
}

#[derive(Insertable)]
#[diesel(table_name = post_signature)]
pub struct NewPostSignature<'a> {
    pub post_id: i32,
    pub signature: &'a [u8],
    pub words: &'a [i32],
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = post_signature)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PostSignature {
    pub post_id: i32,
    pub signature: Vec<u8>,
    pub words: Vec<Option<i32>>,
}

impl PostSignature {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        post_signature::table.count().first(conn)
    }
}
