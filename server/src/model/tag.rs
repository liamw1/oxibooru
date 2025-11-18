use crate::model::tag_category::TagCategory;
use crate::schema::{tag, tag_implication, tag_name, tag_suggestion};
use crate::string::{LargeString, SmallString};
use crate::time::DateTime;
use diesel::dsl::sql;
use diesel::expression::{SqlLiteral, UncheckedBind};
use diesel::pg::Pg;
use diesel::sql_types::Bool;
use diesel::{AsChangeset, Associations, Identifiable, Insertable, Queryable, Selectable};

#[derive(Clone, Copy, Default, Insertable)]
#[diesel(table_name = tag)]
#[diesel(check_for_backend(Pg))]
pub struct NewTag<'a> {
    pub category_id: i64,
    pub description: &'a str,
}

#[derive(Clone, AsChangeset, Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(TagCategory, foreign_key = category_id))]
#[diesel(table_name = tag)]
#[diesel(check_for_backend(Pg))]
pub struct Tag {
    pub id: i64,
    pub category_id: i64,
    pub description: LargeString,
    pub creation_time: DateTime,
    pub last_edit_time: DateTime,
}

#[derive(Insertable)]
#[diesel(table_name = tag_name)]
#[diesel(check_for_backend(Pg))]
pub struct NewTagName<'a> {
    pub tag_id: i64,
    pub order: i32,
    pub name: &'a str,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(Tag))]
#[diesel(table_name = tag_name)]
#[diesel(primary_key(tag_id, order))]
#[diesel(check_for_backend(Pg))]
pub struct TagName {
    pub tag_id: i64,
    pub order: i32,
    pub name: SmallString,
}

impl TagName {
    /// Creates an expression that filters `tag_name` rows to only the primary names.
    /// This exists because certain queries are much slower when using the nearly-equivalent
    /// `tag_name::order.eq(0)` (as much as 15x). Presumably this is because Diesel uses bind
    /// parameters and the Postgres can't optimize these queries as well when it doesn't know
    /// the exact value of `order`.
    pub fn primary() -> SqlLiteral<Bool, UncheckedBind<SqlLiteral<Bool>, tag_name::order>> {
        sql("").bind(tag_name::order).sql(" = 0")
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Tag, foreign_key = parent_id))]
#[diesel(table_name = tag_implication)]
#[diesel(primary_key(parent_id, child_id))]
#[diesel(check_for_backend(Pg))]
pub struct TagImplication {
    pub parent_id: i64,
    pub child_id: i64,
}

diesel::joinable!(tag_implication -> tag (parent_id));

#[derive(Clone, Copy, PartialEq, Eq, Hash, Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Tag, foreign_key = parent_id))]
#[diesel(table_name = tag_suggestion)]
#[diesel(primary_key(parent_id, child_id))]
#[diesel(check_for_backend(Pg))]
pub struct TagSuggestion {
    pub parent_id: i64,
    pub child_id: i64,
}

diesel::joinable!(tag_suggestion -> tag (parent_id));
