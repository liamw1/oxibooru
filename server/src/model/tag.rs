use crate::schema::{tag, tag_category, tag_implication, tag_name, tag_suggestion};
use crate::time::DateTime;
use diesel::pg::Pg;
use diesel::prelude::*;

#[derive(Insertable)]
#[diesel(table_name = tag_category)]
#[diesel(check_for_backend(Pg))]
pub struct NewTagCategory<'a> {
    pub order: i32,
    pub name: &'a str,
    pub color: &'a str,
}

#[derive(AsChangeset, Identifiable, Queryable, Selectable)]
#[diesel(table_name = tag_category)]
#[diesel(check_for_backend(Pg))]
pub struct TagCategory {
    pub id: i32,
    pub order: i32,
    pub name: String,
    pub color: String,
    pub last_edit_time: DateTime,
}

impl TagCategory {
    pub fn from_name(conn: &mut PgConnection, name: &str) -> QueryResult<Self> {
        tag_category::table.filter(tag_category::name.eq(name)).first(conn)
    }
}

#[derive(Clone, Copy, Default, Insertable)]
#[diesel(table_name = tag)]
#[diesel(check_for_backend(Pg))]
pub struct NewTag {
    pub category_id: i32,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(TagCategory, foreign_key = category_id))]
#[diesel(table_name = tag)]
#[diesel(check_for_backend(Pg))]
pub struct Tag {
    pub id: i32,
    pub category_id: i32,
    pub description: String,
    pub creation_time: DateTime,
    pub last_edit_time: DateTime,
}

#[derive(Insertable)]
#[diesel(table_name = tag_name)]
#[diesel(check_for_backend(Pg))]
pub struct NewTagName<'a> {
    pub tag_id: i32,
    pub order: i32,
    pub name: &'a str,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(Tag))]
#[diesel(table_name = tag_name)]
#[diesel(primary_key(tag_id, order))]
#[diesel(check_for_backend(Pg))]
pub struct TagName {
    pub tag_id: i32,
    pub order: i32,
    pub name: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Tag, foreign_key = parent_id))]
#[diesel(table_name = tag_implication)]
#[diesel(primary_key(parent_id, child_id))]
#[diesel(check_for_backend(Pg))]
pub struct TagImplication {
    pub parent_id: i32,
    pub child_id: i32,
}

diesel::joinable!(tag_implication -> tag (parent_id));

#[derive(Clone, Copy, PartialEq, Eq, Hash, Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Tag, foreign_key = parent_id))]
#[diesel(table_name = tag_suggestion)]
#[diesel(primary_key(parent_id, child_id))]
#[diesel(check_for_backend(Pg))]
pub struct TagSuggestion {
    pub parent_id: i32,
    pub child_id: i32,
}

diesel::joinable!(tag_suggestion -> tag (parent_id));
