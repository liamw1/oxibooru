use crate::content::signature::NUM_WORDS;
use crate::model::enums::{MimeType, PostFlags, PostSafety, PostType, Score};
use crate::model::tag::Tag;
use crate::model::user::User;
use crate::schema::{
    post, post_favorite, post_feature, post_note, post_relation, post_score, post_signature, post_tag,
};
use crate::time::DateTime;
use diesel::pg::Pg;
use diesel::prelude::*;
use std::option::Option;

#[derive(Insertable)]
#[diesel(table_name = post)]
#[diesel(check_for_backend(Pg))]
pub struct NewPost<'a> {
    pub user_id: Option<i64>,
    pub file_size: i64,
    pub width: i32,
    pub height: i32,
    pub safety: PostSafety,
    pub type_: PostType,
    pub mime_type: MimeType,
    pub checksum: &'a str,
    pub checksum_md5: &'a str,
    pub flags: PostFlags,
    pub source: Option<&'a str>,
}

#[derive(AsChangeset, Associations, Identifiable, Queryable, Selectable)]
#[diesel(treat_none_as_null = true)]
#[diesel(belongs_to(User))]
#[diesel(table_name = post)]
#[diesel(check_for_backend(Pg))]
pub struct Post {
    pub id: i64,
    pub user_id: Option<i64>,
    pub file_size: i64,
    pub width: i32,
    pub height: i32,
    pub safety: PostSafety,
    pub type_: PostType,
    pub mime_type: MimeType,
    pub checksum: String,
    pub checksum_md5: Option<String>,
    pub flags: PostFlags,
    pub source: Option<String>,
    pub creation_time: DateTime,
    pub last_edit_time: DateTime,
    pub generated_thumbnail_size: i64,
    pub custom_thumbnail_size: i64,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Post, foreign_key = parent_id))]
#[diesel(table_name = post_relation)]
#[diesel(primary_key(parent_id, child_id))]
#[diesel(check_for_backend(Pg))]
pub struct PostRelation {
    pub parent_id: i64,
    pub child_id: i64,
}

diesel::joinable!(post_relation -> post (parent_id));

impl PostRelation {
    pub fn new_pair(id_1: i64, id_2: i64) -> [Self; 2] {
        [PostRelation::new(id_1, id_2), PostRelation::new(id_2, id_1)]
    }

    fn new(parent_id: i64, child_id: i64) -> Self {
        Self { parent_id, child_id }
    }
}

#[derive(Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Post), belongs_to(Tag))]
#[diesel(table_name = post_tag)]
#[diesel(primary_key(post_id, tag_id))]
#[diesel(check_for_backend(Pg))]
pub struct PostTag {
    pub post_id: i64,
    pub tag_id: i64,
}

#[derive(Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Post), belongs_to(User))]
#[diesel(table_name = post_favorite)]
#[diesel(primary_key(post_id, user_id))]
#[diesel(check_for_backend(Pg))]
pub struct PostFavorite {
    pub post_id: i64,
    pub user_id: i64,
    pub time: DateTime,
}

#[derive(Insertable)]
#[diesel(table_name = post_feature)]
#[diesel(check_for_backend(Pg))]
pub struct NewPostFeature {
    pub post_id: i64,
    pub user_id: i64,
    pub time: DateTime,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(Post), belongs_to(User))]
#[diesel(table_name = post_feature)]
#[diesel(check_for_backend(Pg))]
pub struct PostFeature {
    pub id: i64,
    pub post_id: i64,
    pub user_id: i64,
    pub time: DateTime,
}

#[derive(Insertable)]
#[diesel(table_name = post_note)]
#[diesel(check_for_backend(Pg))]
pub struct NewPostNote<'a> {
    pub post_id: i64,
    pub polygon: &'a [f32],
    pub text: &'a str,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(Post))]
#[diesel(table_name = post_note)]
#[diesel(check_for_backend(Pg))]
pub struct PostNote {
    pub id: i64,
    pub post_id: i64,
    pub polygon: Vec<Option<f32>>,
    pub text: String,
}

#[derive(Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Post), belongs_to(User))]
#[diesel(table_name = post_score)]
#[diesel(primary_key(post_id, user_id))]
#[diesel(check_for_backend(Pg))]
pub struct PostScore {
    pub post_id: i64,
    pub user_id: i64,
    pub score: Score,
    pub time: DateTime,
}

#[derive(Insertable)]
#[diesel(table_name = post_signature)]
#[diesel(check_for_backend(Pg))]
pub struct NewPostSignature<'a> {
    pub post_id: i64,
    pub signature: &'a [i64],
    pub words: &'a [i32],
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(Post))]
#[diesel(table_name = post_signature)]
#[diesel(primary_key(post_id))]
#[diesel(check_for_backend(Pg))]
pub struct PostSignature {
    pub post_id: i64,
    pub signature: Vec<Option<i64>>,
}

impl PostSignature {
    pub fn find_similar(conn: &mut PgConnection, words: [i32; NUM_WORDS]) -> QueryResult<Vec<Self>> {
        post_signature::table
            .select(PostSignature::as_select())
            .filter(post_signature::words.overlaps_with(words.as_slice()))
            .load(conn)
    }
}
