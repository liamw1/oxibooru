use crate::schema::tag;
use crate::schema::tag_category;
use crate::schema::tag_implication;
use crate::schema::tag_name;
use crate::schema::tag_suggestion;
use chrono::DateTime;
use chrono::Utc;
use diesel::prelude::*;
use std::option::Option;

#[derive(Insertable)]
#[diesel(table_name = tag_category)]
pub struct NewTagCategory<'a> {
    pub order: i32,
    pub name: &'a str,
    pub color: &'a str,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = tag_category)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct TagCategory {
    pub id: i32,
    pub order: i32,
    pub name: String,
    pub color: String,
}

#[derive(Insertable)]
#[diesel(table_name = tag)]
pub struct NewTag {
    pub category_id: i32,
    pub creation_time: DateTime<Utc>,
    pub last_edit_time: DateTime<Utc>,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = tag)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Tag {
    pub id: i32,
    pub category_id: i32,
    pub description: Option<String>,
    pub creation_time: DateTime<Utc>,
    pub last_edit_time: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = tag_name)]
pub struct NewTagName<'a> {
    pub tag_id: i32,
    pub order: i32,
    pub name: &'a str,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = tag_name)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct TagName {
    pub id: i32,
    pub tag_id: i32,
    pub order: i32,
    pub name: String,
}

#[allow(dead_code)]
type NewTagImplication = TagImplication;

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = tag_implication)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct TagImplication {
    pub parent_id: i32,
    pub child_id: i32,
}

#[allow(dead_code)]
type NewTagSuggestion = TagSuggestion;

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = tag_suggestion)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct TagSuggestion {
    pub parent_id: i32,
    pub child_id: i32,
}
