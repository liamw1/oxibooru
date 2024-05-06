use crate::schema::{tag, tag_category, tag_implication, tag_name, tag_suggestion};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error};
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

    pub fn delete(self, conn: &mut PgConnection) -> QueryResult<()> {
        conn.transaction(|conn| {
            let num_deleted = diesel::delete(tag::table.filter(tag::columns::id.eq(self.id))).execute(conn)?;
            let error_message =
                |msg: String| -> Error { Error::DatabaseError(DatabaseErrorKind::UniqueViolation, Box::new(msg)) };
            match num_deleted {
                0 => Err(error_message(format!("Failed to delete tag: no tag with id {}", self.id))),
                1 => Ok(()),
                _ => Err(error_message(format!("Failed to delete tag: id {} is not unique", self.id))),
            }
        })
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
