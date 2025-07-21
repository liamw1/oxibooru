use crate::schema::tag_category;
use crate::string::SmallString;
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
    pub id: i64,
    pub order: i32,
    pub name: SmallString,
    pub color: SmallString,
    pub last_edit_time: DateTime,
}
