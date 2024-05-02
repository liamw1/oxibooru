use crate::schema::post;
use crate::schema::post_favorite;
use crate::schema::post_feature;
use crate::schema::post_note;
use crate::schema::post_relation;
use crate::schema::post_score;
use crate::schema::post_signature;
use crate::schema::post_tag;
use chrono::DateTime;
use chrono::Utc;
use diesel::prelude::*;
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

pub type NewPostRelation = PostRelation;

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = post_relation)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PostRelation {
    pub parent_id: i32,
    pub child_id: i32,
}

pub type NewPostTag = PostTag;

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = post_tag)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PostTag {
    pub post_id: i32,
    pub tag_id: i32,
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
