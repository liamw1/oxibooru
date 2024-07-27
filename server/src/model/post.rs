use crate::model::enums::{MimeType, PostSafety, PostType};
use crate::model::pool::{Pool, PoolPost};
use crate::model::tag::Tag;
use crate::model::user::User;
use crate::model::TableName;
use crate::schema::{
    pool, post, post_favorite, post_feature, post_note, post_relation, post_score, post_signature, post_tag,
};
use crate::util;
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

impl TableName for Post {
    fn table_name() -> &'static str {
        "post"
    }
}

impl Post {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        post::table.count().first(conn)
    }

    pub fn score(&self, conn: &mut PgConnection) -> QueryResult<i64> {
        PostScore::belonging_to(self)
            .select(diesel::dsl::sum(post_score::score))
            .first::<Option<_>>(conn)
            .map(Option::unwrap_or_default)
    }

    pub fn favorite_count(&self, conn: &mut PgConnection) -> QueryResult<i64> {
        PostFavorite::belonging_to(self).count().first(conn)
    }

    pub fn feature_count(&self, conn: &mut PgConnection) -> QueryResult<i64> {
        PostFeature::belonging_to(self).count().first(conn)
    }

    pub fn tag_count(&self, conn: &mut PgConnection) -> QueryResult<i64> {
        PostTag::belonging_to(self).count().first(conn)
    }

    pub fn pools_in(&self, conn: &mut PgConnection) -> QueryResult<Vec<Pool>> {
        PoolPost::belonging_to(self)
            .inner_join(pool::table)
            .select(Pool::as_select())
            .load(conn)
    }

    pub fn related_posts(&self, conn: &mut PgConnection) -> QueryResult<Vec<Self>> {
        PostRelation::belonging_to(self)
            .inner_join(post::table.on(post::id.eq(post_relation::child_id)))
            .select(Self::as_select())
            .load(conn)
    }

    pub fn add_tag(&self, conn: &mut PgConnection, tag: &Tag) -> QueryResult<PostTag> {
        let new_post_tag = NewPostTag {
            post_id: self.id,
            tag_id: tag.id,
        };
        diesel::insert_into(post_tag::table)
            .values(new_post_tag)
            .returning(PostTag::as_returning())
            .get_result(conn)
    }

    pub fn add_relation(&self, conn: &mut PgConnection, related_post: &Self) -> QueryResult<PostRelation> {
        let new_post_relation = NewPostRelation {
            parent_id: self.id,
            child_id: related_post.id,
        };
        diesel::insert_into(post_relation::table)
            .values(new_post_relation)
            .returning(PostRelation::as_returning())
            .get_result(conn)
    }

    pub fn delete(self, conn: &mut PgConnection) -> QueryResult<()> {
        util::delete(conn, &self)
    }
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

impl PostRelation {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        post_relation::table.count().first(conn)
    }
}

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

impl PostTag {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        post_tag::table.count().first(conn)
    }
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

impl PostFeature {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        post_feature::table.count().first(conn)
    }
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

impl PostNote {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        post_note::table.count().first(conn)
    }
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
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        post_signature::table.count().first(conn)
    }

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
    use crate::model::comment::Comment;
    use crate::model::tag::Tag;
    use crate::model::user::User;
    use crate::test::*;

    #[test]
    fn save_post() {
        let post = test_transaction(|conn: &mut PgConnection| {
            create_test_user(conn, TEST_USERNAME).and_then(|user| create_test_post(conn, &user))
        });

        assert_eq!(post.safety, PostSafety::Safe);
    }

    #[test]
    fn cascade_deletions() {
        test_transaction(|conn: &mut PgConnection| {
            let user_count = User::count(conn)?;
            let tag_count = Tag::count(conn)?;
            let comment_count = Comment::count(conn)?;
            let post_count = Post::count(conn)?;
            let post_tag_count = PostTag::count(conn)?;
            let post_relation_count = PostRelation::count(conn)?;
            let post_score_count = PostScore::count(conn)?;
            let post_note_count = PostNote::count(conn)?;
            let post_feature_count = PostFeature::count(conn)?;
            let post_favorite_count = PostFavorite::count(conn)?;
            let post_signature_count = PostSignature::count(conn)?;

            let user = create_test_user(conn, TEST_USERNAME)?;
            let tag1 = Tag::new(conn)?;
            let tag2 = Tag::new(conn)?;
            let post = create_test_post(conn, &user)?;
            let related_post1 = create_test_post(conn, &user)?;
            let related_post2 = create_test_post(conn, &user)?;
            let comment = user.add_comment(conn, &post, "This is a test comment")?;

            post.add_tag(conn, &tag1)?;
            post.add_tag(conn, &tag2)?;
            post.add_relation(conn, &related_post1)?;
            post.add_relation(conn, &related_post2)?;
            create_test_post_note(conn, &post)?;
            create_test_post_signature(conn, &post)?;

            user.like_comment(conn, &comment)?;
            user.like_post(conn, &post)?;
            user.favorite_post(conn, &post)?;
            user.feature_post(conn, &post)?;

            assert_eq!(post.related_posts(conn)?.len(), 2);
            assert_eq!(User::count(conn)?, user_count + 1);
            assert_eq!(Tag::count(conn)?, tag_count + 2);
            assert_eq!(Comment::count(conn)?, comment_count + 1);
            assert_eq!(Post::count(conn)?, post_count + 3);
            assert_eq!(PostTag::count(conn)?, post_tag_count + 2);
            assert_eq!(PostRelation::count(conn)?, post_relation_count + 2);
            assert_eq!(PostScore::count(conn)?, post_score_count + 1);
            assert_eq!(PostNote::count(conn)?, post_note_count + 1);
            assert_eq!(PostFeature::count(conn)?, post_feature_count + 1);
            assert_eq!(PostFavorite::count(conn)?, post_favorite_count + 1);
            assert_eq!(PostSignature::count(conn)?, post_signature_count + 1);

            post.delete(conn)?;

            assert_eq!(User::count(conn)?, user_count + 1);
            assert_eq!(Tag::count(conn)?, tag_count + 2);
            assert_eq!(Comment::count(conn)?, comment_count);
            assert_eq!(Post::count(conn)?, post_count + 2);
            assert_eq!(PostTag::count(conn)?, post_tag_count);
            assert_eq!(PostRelation::count(conn)?, post_relation_count);
            assert_eq!(PostScore::count(conn)?, post_score_count);
            assert_eq!(PostNote::count(conn)?, post_note_count);
            assert_eq!(PostFeature::count(conn)?, post_feature_count);
            assert_eq!(PostFavorite::count(conn)?, post_favorite_count);
            assert_eq!(PostSignature::count(conn)?, post_signature_count);

            Ok(())
        });
    }

    #[test]
    fn track_tag_count() {
        test_transaction(|conn: &mut PgConnection| {
            let post = create_test_user(conn, TEST_USERNAME).and_then(|user| create_test_post(conn, &user))?;
            let tag1 = Tag::new(conn)?;
            let tag2 = Tag::new(conn)?;

            post.add_tag(conn, &tag1)?;
            post.add_tag(conn, &tag2)?;

            assert_eq!(post.tag_count(conn)?, 2);

            tag1.delete(conn)?;

            assert_eq!(post.tag_count(conn)?, 1);

            tag2.delete(conn)?;

            assert_eq!(post.tag_count(conn)?, 0);

            Ok(())
        });
    }
}
