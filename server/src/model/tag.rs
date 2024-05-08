use crate::model::post::PostTag;
use crate::schema::{tag, tag_category, tag_implication, tag_name, tag_suggestion};
use crate::util;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use std::option::Option;

#[derive(Insertable)]
#[diesel(table_name = tag_category)]
pub struct NewTagCategory<'a> {
    pub order: i32,
    pub name: &'a str,
    pub color: &'a str,
}

#[derive(Identifiable, Queryable, Selectable)]
#[diesel(table_name = tag_category)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct TagCategory {
    pub id: i32,
    pub order: i32,
    pub name: String,
    pub color: String,
}

impl TagCategory {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        tag_category::table.count().first(conn)
    }
}

#[derive(Insertable)]
#[diesel(table_name = tag)]
pub struct NewTag {
    pub category_id: i32,
    pub creation_time: DateTime<Utc>,
    pub last_edit_time: DateTime<Utc>,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(TagCategory, foreign_key = category_id))]
#[diesel(table_name = tag)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Tag {
    pub id: i32,
    pub category_id: i32,
    pub description: Option<String>,
    pub creation_time: DateTime<Utc>,
    pub last_edit_time: DateTime<Utc>,
}

impl Tag {
    pub fn new(conn: &mut PgConnection) -> QueryResult<Tag> {
        let now = Utc::now();
        let new_tag = NewTag {
            category_id: 0,
            creation_time: now,
            last_edit_time: now,
        };
        diesel::insert_into(tag::table)
            .values(&new_tag)
            .returning(Tag::as_returning())
            .get_result(conn)
    }

    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        tag::table.count().first(conn)
    }

    pub fn post_count(&self, conn: &mut PgConnection) -> QueryResult<i64> {
        PostTag::belonging_to(self).count().first(conn)
    }

    pub fn primary_name(&self, conn: &mut PgConnection) -> QueryResult<String> {
        TagName::belonging_to(self)
            .select(tag_name::columns::name)
            .filter(tag_name::columns::order.eq(0))
            .first(conn)
    }

    pub fn names(&self, conn: &mut PgConnection) -> QueryResult<Vec<String>> {
        TagName::belonging_to(self).select(tag_name::columns::name).load(conn)
    }

    pub fn implications(&self, conn: &mut PgConnection) -> QueryResult<Vec<Tag>> {
        TagImplication::belonging_to(self)
            .inner_join(tag::table.on(tag::columns::id.eq(tag_implication::columns::child_id)))
            .select(Tag::as_select())
            .load(conn)
    }

    pub fn suggestions(&self, conn: &mut PgConnection) -> QueryResult<Vec<Tag>> {
        TagSuggestion::belonging_to(self)
            .inner_join(tag::table.on(tag::columns::id.eq(tag_suggestion::columns::child_id)))
            .select(Tag::as_select())
            .load(conn)
    }

    pub fn add_name(&self, conn: &mut PgConnection, name: &str) -> QueryResult<TagName> {
        let name_count = TagName::belonging_to(self).count().first::<i64>(conn)?;
        let new_tag_name = NewTagName {
            tag_id: self.id,
            order: name_count as i32,
            name,
        };
        diesel::insert_into(tag_name::table)
            .values(&new_tag_name)
            .returning(TagName::as_returning())
            .get_result(conn)
    }

    pub fn add_implication(&self, conn: &mut PgConnection, implied_tag: &Tag) -> QueryResult<TagImplication> {
        let new_tag_implication = NewTagImplication {
            parent_id: self.id,
            child_id: implied_tag.id,
        };
        diesel::insert_into(tag_implication::table)
            .values(&new_tag_implication)
            .returning(TagImplication::as_returning())
            .get_result(conn)
    }

    pub fn add_suggestion(&self, conn: &mut PgConnection, implied_tag: &Tag) -> QueryResult<TagSuggestion> {
        let new_tag_suggestion = NewTagSuggestion {
            parent_id: self.id,
            child_id: implied_tag.id,
        };
        diesel::insert_into(tag_suggestion::table)
            .values(&new_tag_suggestion)
            .returning(TagSuggestion::as_returning())
            .get_result(conn)
    }

    pub fn delete(self, conn: &mut PgConnection) -> QueryResult<()> {
        conn.transaction(|conn| util::validate_uniqueness("tag", diesel::delete(&self).execute(conn)?))
    }
}

#[derive(Insertable)]
#[diesel(table_name = tag_name)]
pub struct NewTagName<'a> {
    pub tag_id: i32,
    pub order: i32,
    pub name: &'a str,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(Tag))]
#[diesel(table_name = tag_name)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct TagName {
    pub id: i32,
    pub tag_id: i32,
    pub order: i32,
    pub name: String,
}

impl TagName {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        tag_name::table.count().first(conn)
    }
}

pub type NewTagImplication = TagImplication;

#[derive(Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Tag, foreign_key = parent_id))]
#[diesel(table_name = tag_implication)]
#[diesel(primary_key(parent_id, child_id))]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct TagImplication {
    pub parent_id: i32,
    pub child_id: i32,
}

impl TagImplication {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        tag_implication::table.count().first(conn)
    }
}

pub type NewTagSuggestion = TagSuggestion;

#[derive(Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Tag, foreign_key = parent_id))]
#[diesel(table_name = tag_suggestion)]
#[diesel(primary_key(parent_id, child_id))]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct TagSuggestion {
    pub parent_id: i32,
    pub child_id: i32,
}

impl TagSuggestion {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        tag_suggestion::table.count().first(conn)
    }
}

#[cfg(test)]
mod test {
    use super::{Tag, TagImplication, TagName, TagSuggestion};
    use crate::test::*;
    use diesel::prelude::*;
    use diesel::result::Error;

    #[test]
    fn test_saving_tag() {
        establish_connection_or_panic().test_transaction::<_, Error, _>(|conn| {
            let tag = Tag::new(conn)?;
            let implication1 = Tag::new(conn)?;
            let implication2 = Tag::new(conn)?;
            let suggestion1 = Tag::new(conn)?;
            let suggestion2 = Tag::new(conn)?;

            tag.add_implication(conn, &implication1)?;
            tag.add_implication(conn, &implication2)?;
            tag.add_suggestion(conn, &suggestion1)?;
            tag.add_suggestion(conn, &suggestion2)?;
            tag.add_name(conn, "alias1")?;
            tag.add_name(conn, "alias2")?;
            implication1.add_name(conn, "imp1")?;
            implication2.add_name(conn, "imp2")?;
            suggestion1.add_name(conn, "sug1")?;
            suggestion2.add_name(conn, "sug2")?;

            assert_eq!(tag.names(conn)?, vec!["alias1", "alias2"], "Incorrect tag names");

            let implication_names = tag
                .implications(conn)?
                .into_iter()
                .map(|implication| implication.primary_name(conn))
                .collect::<QueryResult<Vec<_>>>()?;
            let suggestion_names = tag
                .suggestions(conn)?
                .into_iter()
                .map(|suggestion| suggestion.primary_name(conn))
                .collect::<QueryResult<Vec<_>>>()?;

            assert_eq!(implication_names, vec!["imp1", "imp2"], "Incorrect implication names");
            assert_eq!(suggestion_names, vec!["sug1", "sug2"], "Incorrect suggestion names");

            Ok(())
        })
    }

    #[test]
    fn test_cascade_deletions() {
        establish_connection_or_panic().test_transaction::<_, Error, _>(|conn| {
            let tag_count = Tag::count(conn)?;
            let tag_name_count = TagName::count(conn)?;
            let tag_implication_count = TagImplication::count(conn)?;
            let tag_suggestion_count = TagSuggestion::count(conn)?;

            let tag = Tag::new(conn)?;
            let implication1 = Tag::new(conn)?;
            let implication2 = Tag::new(conn)?;
            let suggestion1 = Tag::new(conn)?;
            let suggestion2 = Tag::new(conn)?;

            tag.add_implication(conn, &implication1)?;
            tag.add_implication(conn, &implication2)?;
            tag.add_suggestion(conn, &suggestion1)?;
            tag.add_suggestion(conn, &suggestion2)?;
            tag.add_name(conn, "alias1")?;
            tag.add_name(conn, "alias2")?;
            implication1.add_name(conn, "imp1")?;
            implication2.add_name(conn, "imp2")?;
            suggestion1.add_name(conn, "sug1")?;
            suggestion2.add_name(conn, "sug2")?;

            assert_eq!(Tag::count(conn)?, tag_count + 5, "Tag insertion failed");
            assert_eq!(TagName::count(conn)?, tag_name_count + 6, "Tag name insertion failed");
            assert_eq!(TagImplication::count(conn)?, tag_implication_count + 2, "Tag implication insertion failed");
            assert_eq!(TagSuggestion::count(conn)?, tag_suggestion_count + 2, "Tag suggestion insertion failed");

            tag.delete(conn)?;

            assert_eq!(Tag::count(conn)?, tag_count + 4, "Only one tag should have been deleted");
            assert_eq!(TagName::count(conn)?, tag_name_count + 4, "Only two tag names should have been deleted");
            assert_eq!(TagImplication::count(conn)?, tag_implication_count, "Tag implication cascade deletion failed");
            assert_eq!(TagSuggestion::count(conn)?, tag_suggestion_count, "Tag suggestion cascade deletion failed");

            Ok(())
        })
    }

    #[test]
    fn test_tracking_post_count() {
        establish_connection_or_panic().test_transaction::<_, Error, _>(|conn| {
            let user = create_test_user(conn, "test_user")?;
            let post1 = create_test_post(conn, &user)?;
            let post2 = create_test_post(conn, &user)?;
            let tag1 = Tag::new(conn)?;
            let tag2 = Tag::new(conn)?;

            post1.add_tag(conn, &tag1)?;
            post2.add_tag(conn, &tag1)?;
            post2.add_tag(conn, &tag2)?;

            assert_eq!(post1.tag_count(conn)?, 1, "Post should have one tag");
            assert_eq!(post2.tag_count(conn)?, 2, "Post should have two tags");
            assert_eq!(tag1.post_count(conn)?, 2, "Tag should be on two posts");
            assert_eq!(tag2.post_count(conn)?, 1, "Tag should be on one post");

            post2.delete(conn)?;

            assert_eq!(tag1.post_count(conn)?, 1, "Tag should now be on one post");
            assert_eq!(tag2.post_count(conn)?, 0, "Tag should now be on no posts");

            post1.delete(conn)?;

            assert_eq!(tag1.post_count(conn)?, 0, "Both tags should now be on no posts");

            Ok(())
        })
    }
}
