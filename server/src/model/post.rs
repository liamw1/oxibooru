use crate::model::enums::{MimeType, PostSafety, PostType};
use crate::model::tag::Tag;
use crate::model::user::User;
use crate::schema::{
    post, post_favorite, post_feature, post_note, post_relation, post_score, post_signature, post_tag,
};
use crate::util::DateTime;
use diesel::deserialize::{self, FromSql, FromSqlRow};
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::{Array, Int4, Integer};
use diesel::AsExpression;
use std::option::Option;

#[derive(Insertable)]
#[diesel(table_name = post)]
pub struct NewPost<'a> {
    pub user_id: Option<i32>,
    pub file_size: i64,
    pub width: i32,
    pub height: i32,
    pub safety: PostSafety,
    pub type_: PostType,
    pub mime_type: MimeType,
    pub checksum: &'a str,
    pub source: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, Associations, Selectable, AsExpression, FromSqlRow)]
#[diesel(sql_type = Integer)]
#[diesel(belongs_to(Post, foreign_key = id))]
#[diesel(table_name = post)]
#[diesel(check_for_backend(Pg))]
pub struct PostId {
    pub id: i32,
}

impl ToSql<Integer, Pg> for PostId
where
    i32: ToSql<Integer, Pg>,
{
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        <i32 as ToSql<Integer, Pg>>::to_sql(&self.id, &mut out.reborrow())
    }
}

impl FromSql<Integer, Pg> for PostId
where
    i32: FromSql<Integer, Pg>,
{
    fn from_sql(bytes: <Pg as diesel::backend::Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        i32::from_sql(bytes).map(|id| PostId { id })
    }
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(User))]
#[diesel(table_name = post)]
#[diesel(check_for_backend(Pg))]
pub struct Post {
    pub id: i32,
    pub user_id: Option<i32>,
    pub file_size: i64,
    pub width: i32,
    pub height: i32,
    pub safety: PostSafety,
    pub type_: PostType,
    pub mime_type: MimeType,
    pub checksum: String,
    pub checksum_md5: Option<String>,
    pub flags: Option<String>,
    pub source: Option<String>,
    pub creation_time: DateTime,
    pub last_edit_time: DateTime,
}

pub type NewPostRelation = PostRelation;

#[derive(Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Post, foreign_key = parent_id))]
#[diesel(table_name = post_relation)]
#[diesel(primary_key(parent_id, child_id))]
#[diesel(check_for_backend(Pg))]
pub struct PostRelation {
    pub parent_id: i32,
    pub child_id: i32,
}

diesel::joinable!(post_relation -> post (parent_id));

pub type NewPostTag = PostTag;

#[derive(Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Post), belongs_to(Tag))]
#[diesel(table_name = post_tag)]
#[diesel(primary_key(post_id, tag_id))]
#[diesel(check_for_backend(Pg))]
pub struct PostTag {
    pub post_id: i32,
    pub tag_id: i32,
}

pub type NewPostFavorite = PostFavorite;

#[derive(Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Post), belongs_to(User))]
#[diesel(table_name = post_favorite)]
#[diesel(primary_key(post_id, user_id))]
#[diesel(check_for_backend(Pg))]
pub struct PostFavorite {
    pub post_id: i32,
    pub user_id: i32,
    pub time: DateTime,
}

#[derive(Insertable)]
#[diesel(table_name = post_feature)]
pub struct NewPostFeature {
    pub post_id: i32,
    pub user_id: i32,
    pub time: DateTime,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(Post), belongs_to(User))]
#[diesel(table_name = post_feature)]
#[diesel(check_for_backend(Pg))]
pub struct PostFeature {
    pub id: i32,
    pub post_id: i32,
    pub user_id: i32,
    pub time: DateTime,
}

#[derive(Insertable)]
#[diesel(table_name = post_note)]
pub struct NewPostNote<'a> {
    pub post_id: i32,
    pub polygon: &'a [u8],
    pub text: String,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(Post))]
#[diesel(table_name = post_note)]
#[diesel(check_for_backend(Pg))]
pub struct PostNote {
    pub id: i32,
    pub post_id: i32,
    pub polygon: Vec<u8>,
    pub text: String,
}

pub type NewPostScore = PostScore;

#[derive(Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Post), belongs_to(User))]
#[diesel(table_name = post_score)]
#[diesel(primary_key(post_id, user_id))]
#[diesel(check_for_backend(Pg))]
pub struct PostScore {
    pub post_id: i32,
    pub user_id: i32,
    pub score: i32,
    pub time: DateTime,
}

#[derive(Insertable)]
#[diesel(table_name = post_signature)]
pub struct NewPostSignature<'a> {
    pub post_id: i32,
    pub signature: &'a [u8],
    pub words: &'a [i32],
}

#[derive(Associations, Identifiable, Queryable, QueryableByName, Selectable)]
#[diesel(belongs_to(Post))]
#[diesel(table_name = post_signature)]
#[diesel(primary_key(post_id))]
#[diesel(check_for_backend(Pg))]
pub struct PostSignature {
    pub post_id: i32,
    pub signature: Vec<u8>,
}

impl PostSignature {
    pub fn find_similar(conn: &mut PgConnection, words: Vec<i32>) -> QueryResult<Vec<Self>> {
        diesel::sql_query(
            "SELECT s.post_id, s.signature
             FROM post_signature AS s, unnest(s.words, $1) AS a(word, query)
             WHERE a.word = a.query
             GROUP BY s.post_id
             ORDER BY count(a.query) DESC;",
        )
        .bind::<Array<Int4>, _>(words)
        .load(conn)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test::*;

    #[test]
    fn save_post() {
        let post = test_transaction(|conn: &mut PgConnection| {
            create_test_user(conn, TEST_USERNAME).and_then(|user| create_test_post(conn, &user))
        });

        assert_eq!(post.safety, PostSafety::Safe);
    }
}
