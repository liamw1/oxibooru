use crate::model::pool::{Pool, PoolPost};
use crate::model::tag::Tag;
use crate::model::user::User;
use crate::schema::{
    pool, post, post_favorite, post_feature, post_note, post_relation, post_score, post_signature, post_tag,
};
use crate::util;
use chrono::{DateTime, Utc};
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
}

#[derive(Debug, PartialEq, Eq, Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(User))]
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
}

impl Post {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        post::table.count().first(conn)
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

    pub fn related_posts(&self, conn: &mut PgConnection) -> QueryResult<Vec<Post>> {
        PostRelation::belonging_to(self)
            .inner_join(post::table.on(post::columns::id.eq(post_relation::columns::child_id)))
            .select(Post::as_select())
            .load(conn)
    }

    pub fn add_tag(&self, conn: &mut PgConnection, tag: &Tag) -> QueryResult<PostTag> {
        let new_post_tag = NewPostTag {
            post_id: self.id,
            tag_id: tag.id,
        };
        diesel::insert_into(post_tag::table)
            .values(&new_post_tag)
            .returning(PostTag::as_returning())
            .get_result(conn)
    }

    pub fn add_relation(&self, conn: &mut PgConnection, related_post: &Post) -> QueryResult<PostRelation> {
        let new_post_relation = NewPostRelation {
            parent_id: self.id,
            child_id: related_post.id,
        };
        diesel::insert_into(post_relation::table)
            .values(&new_post_relation)
            .returning(PostRelation::as_returning())
            .get_result(conn)
    }

    pub fn delete(self, conn: &mut PgConnection) -> QueryResult<()> {
        conn.transaction(|conn| util::validate_deletion("post", diesel::delete(&self).execute(conn)?))
    }
}

pub type NewPostRelation = PostRelation;

#[derive(Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Post, foreign_key = parent_id))]
#[diesel(table_name = post_relation)]
#[diesel(primary_key(parent_id, child_id))]
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

#[derive(Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Post), belongs_to(Tag))]
#[diesel(table_name = post_tag)]
#[diesel(primary_key(post_id, tag_id))]
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

#[derive(Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Post), belongs_to(User))]
#[diesel(table_name = post_favorite)]
#[diesel(primary_key(post_id, user_id))]
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

#[derive(Debug, PartialEq, Eq, Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(Post), belongs_to(User))]
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
pub struct NewPostNote<'a> {
    pub post_id: i32,
    pub polygon: &'a [u8],
    pub text: String,
}

#[derive(Debug, PartialEq, Eq, Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(Post))]
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

#[derive(Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Post), belongs_to(User))]
#[diesel(table_name = post_score)]
#[diesel(primary_key(post_id, user_id))]
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

#[derive(Debug, PartialEq, Eq, Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(Post))]
#[diesel(table_name = post_signature)]
#[diesel(primary_key(post_id))]
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::model::comment::Comment;
    use crate::model::tag::Tag;
    use crate::model::user::User;
    use crate::test::*;

    #[test]
    fn test_saving_post() {
        let post = test_transaction(|conn: &mut PgConnection| {
            create_test_user(conn, TEST_USERNAME).and_then(|user| create_test_post(conn, &user))
        });

        assert_eq!(post.safety, "safe");
    }

    #[test]
    fn test_cascade_deletions() {
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
    fn test_tracking_tag_count() {
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
