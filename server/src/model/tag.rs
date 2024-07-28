use crate::schema::{tag, tag_category, tag_implication, tag_name, tag_suggestion};
use crate::util::DateTime;
use diesel::deserialize::{self, FromSql, FromSqlRow};
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::Integer;
use diesel::AsExpression;
use serde::Serialize;

#[derive(Insertable)]
#[diesel(table_name = tag_category)]
pub struct NewTagCategory<'a> {
    pub order: i32,
    pub name: &'a str,
    pub color: &'a str,
}

#[derive(Identifiable, Queryable, Selectable)]
#[diesel(table_name = tag_category)]
#[diesel(check_for_backend(Pg))]
pub struct TagCategory {
    pub id: i32,
    pub order: i32,
    pub name: String,
    pub color: String,
    pub last_edit_time: DateTime,
}

#[derive(Clone, Copy, Insertable)]
#[diesel(table_name = tag)]
pub struct NewTag {
    pub category_id: i32,
}

impl Default for NewTag {
    fn default() -> Self {
        Self { category_id: 0 }
    }
}

#[derive(Debug, Clone, Copy, Associations, Selectable, AsExpression, FromSqlRow)]
#[diesel(sql_type = Integer)]
#[diesel(belongs_to(Tag, foreign_key = id))]
#[diesel(table_name = tag)]
#[diesel(check_for_backend(Pg))]
pub struct TagId {
    pub id: i32,
}

impl ToSql<Integer, Pg> for TagId
where
    i32: ToSql<Integer, Pg>,
{
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        <i32 as ToSql<Integer, Pg>>::to_sql(&self.id, &mut out.reborrow())
    }
}

impl FromSql<Integer, Pg> for TagId
where
    i32: FromSql<Integer, Pg>,
{
    fn from_sql(bytes: <Pg as diesel::backend::Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        i32::from_sql(bytes).map(|id| TagId { id })
    }
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

impl Tag {
    pub fn from_name(conn: &mut PgConnection, name: &str) -> QueryResult<Self> {
        tag::table
            .select(Tag::as_select())
            .inner_join(tag_name::table)
            .filter(tag_name::name.eq(name))
            .first(conn)
    }
}

#[derive(Insertable)]
#[diesel(table_name = tag_name)]
pub struct NewTagName<'a> {
    pub tag_id: i32,
    pub order: i32,
    pub name: &'a str,
}

#[derive(Serialize, Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(Tag))]
#[diesel(table_name = tag_name)]
#[diesel(check_for_backend(Pg))]
#[serde(transparent)]
pub struct TagName {
    #[serde(skip)]
    pub id: i32,
    #[serde(skip)]
    pub tag_id: i32,
    #[serde(skip)]
    pub order: i32,
    pub name: String,
}

pub type NewTagImplication = TagImplication;

#[derive(Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Tag, foreign_key = parent_id))]
#[diesel(table_name = tag_implication)]
#[diesel(primary_key(parent_id, child_id))]
#[diesel(check_for_backend(Pg))]
pub struct TagImplication {
    pub parent_id: i32,
    pub child_id: i32,
}

diesel::joinable!(tag_implication -> tag (parent_id));

pub type NewTagSuggestion = TagSuggestion;

#[derive(Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Tag, foreign_key = parent_id))]
#[diesel(table_name = tag_suggestion)]
#[diesel(primary_key(parent_id, child_id))]
#[diesel(check_for_backend(Pg))]
pub struct TagSuggestion {
    pub parent_id: i32,
    pub child_id: i32,
}

diesel::joinable!(tag_suggestion -> tag (parent_id));
