use crate::model::post::PostTag;
use crate::model::TableName;
use crate::schema::{tag, tag_category, tag_implication, tag_name, tag_suggestion};
use crate::util;
use crate::util::DateTime;
use diesel::prelude::*;
use std::option::Option;

#[derive(Insertable)]
#[diesel(table_name = tag_category)]
pub struct NewTagCategory<'a> {
    pub order: i32,
    pub name: &'a str,
    pub color: &'a str,
}

#[derive(Debug, PartialEq, Eq, Identifiable, Queryable, Selectable)]
#[diesel(table_name = tag_category)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct TagCategory {
    pub id: i32,
    pub order: i32,
    pub name: String,
    pub color: String,
    pub last_edit_time: DateTime,
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
}

#[derive(Debug, PartialEq, Eq, Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(TagCategory, foreign_key = category_id))]
#[diesel(table_name = tag)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Tag {
    pub id: i32,
    pub category_id: i32,
    pub description: Option<String>,
    pub creation_time: DateTime,
}

impl TableName for Tag {
    fn table_name() -> &'static str {
        "tag"
    }
}

impl Tag {
    pub fn new(conn: &mut PgConnection) -> QueryResult<Self> {
        let new_tag = NewTag { category_id: 0 };
        diesel::insert_into(tag::table)
            .values(&new_tag)
            .returning(Self::as_returning())
            .get_result(conn)
    }

    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        tag::table.count().first(conn)
    }

    pub fn usages(&self, conn: &mut PgConnection) -> QueryResult<i64> {
        PostTag::belonging_to(self).count().first(conn)
    }

    pub fn primary_name(&self, conn: &mut PgConnection) -> QueryResult<String> {
        TagName::belonging_to(self)
            .select(tag_name::name)
            .filter(tag_name::order.eq(0))
            .first(conn)
    }

    pub fn names(&self, conn: &mut PgConnection) -> QueryResult<Vec<String>> {
        TagName::belonging_to(self).select(tag_name::name).load(conn)
    }

    pub fn implications(&self, conn: &mut PgConnection) -> QueryResult<Vec<Self>> {
        TagImplication::belonging_to(self)
            .inner_join(tag::table.on(tag::id.eq(tag_implication::child_id)))
            .select(Self::as_select())
            .load(conn)
    }

    pub fn suggestions(&self, conn: &mut PgConnection) -> QueryResult<Vec<Self>> {
        TagSuggestion::belonging_to(self)
            .inner_join(tag::table.on(tag::id.eq(tag_suggestion::child_id)))
            .select(Self::as_select())
            .load(conn)
    }

    pub fn add_name(&self, conn: &mut PgConnection, name: &str) -> QueryResult<TagName> {
        let name_count: i64 = TagName::belonging_to(self).count().first::<i64>(conn)?;
        let new_tag_name = NewTagName {
            tag_id: self.id,
            order: i32::try_from(name_count).unwrap(),
            name,
        };
        diesel::insert_into(tag_name::table)
            .values(&new_tag_name)
            .returning(TagName::as_returning())
            .get_result(conn)
    }

    pub fn add_implication(&self, conn: &mut PgConnection, implied_tag: &Self) -> QueryResult<TagImplication> {
        let new_tag_implication = NewTagImplication {
            parent_id: self.id,
            child_id: implied_tag.id,
        };
        diesel::insert_into(tag_implication::table)
            .values(&new_tag_implication)
            .returning(TagImplication::as_returning())
            .get_result(conn)
    }

    pub fn add_suggestion(&self, conn: &mut PgConnection, implied_tag: &Self) -> QueryResult<TagSuggestion> {
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
        util::delete(conn, &self)
    }
}

#[derive(Insertable)]
#[diesel(table_name = tag_name)]
pub struct NewTagName<'a> {
    pub tag_id: i32,
    pub order: i32,
    pub name: &'a str,
}

#[derive(Debug, PartialEq, Eq, Associations, Identifiable, Queryable, Selectable)]
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

#[derive(Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, Selectable)]
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

#[derive(Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, Selectable)]
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
    use super::*;
    use crate::test::*;

    #[test]
    fn save_tag() {
        test_transaction(|conn: &mut PgConnection| {
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

            assert_eq!(tag.names(conn)?, vec!["alias1", "alias2"]);

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

            assert_eq!(implication_names, vec!["imp1", "imp2"]);
            assert_eq!(suggestion_names, vec!["sug1", "sug2"]);

            Ok(())
        });
    }

    #[test]
    fn cascade_deletions() {
        test_transaction(|conn: &mut PgConnection| {
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

            assert_eq!(Tag::count(conn)?, tag_count + 5);
            assert_eq!(TagName::count(conn)?, tag_name_count + 6);
            assert_eq!(TagImplication::count(conn)?, tag_implication_count + 2);
            assert_eq!(TagSuggestion::count(conn)?, tag_suggestion_count + 2);

            tag.delete(conn)?;

            assert_eq!(Tag::count(conn)?, tag_count + 4);
            assert_eq!(TagName::count(conn)?, tag_name_count + 4);
            assert_eq!(TagImplication::count(conn)?, tag_implication_count);
            assert_eq!(TagSuggestion::count(conn)?, tag_suggestion_count);

            Ok(())
        });
    }

    #[test]
    fn track_post_count() {
        test_transaction(|conn: &mut PgConnection| {
            let user = create_test_user(conn, "test_user")?;
            let post1 = create_test_post(conn, &user)?;
            let post2 = create_test_post(conn, &user)?;
            let tag1 = Tag::new(conn)?;
            let tag2 = Tag::new(conn)?;

            post1.add_tag(conn, &tag1)?;
            post2.add_tag(conn, &tag1)?;
            post2.add_tag(conn, &tag2)?;

            assert_eq!(post1.tag_count(conn)?, 1);
            assert_eq!(post2.tag_count(conn)?, 2);
            assert_eq!(tag1.usages(conn)?, 2);
            assert_eq!(tag2.usages(conn)?, 1);

            post2.delete(conn)?;

            assert_eq!(tag1.usages(conn)?, 1);
            assert_eq!(tag2.usages(conn)?, 0);

            post1.delete(conn)?;

            assert_eq!(tag1.usages(conn)?, 0);

            Ok(())
        });
    }
}
