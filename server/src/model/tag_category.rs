use crate::schema::tag_category;
use crate::string::SmallString;
use crate::time::DateTime;
use diesel::dsl::sql;
use diesel::expression::{SqlLiteral, UncheckedBind};
use diesel::pg::Pg;
use diesel::sql_types::Bool;
use diesel::{AsChangeset, Identifiable, Insertable, Queryable, Selectable};

#[derive(Insertable)]
#[diesel(table_name = tag_category)]
#[diesel(check_for_backend(Pg))]
pub struct NewTagCategory<'a> {
    pub order: i32,
    pub name: &'a str,
    pub color: &'a str,
}

#[derive(Clone, PartialEq, Eq, AsChangeset, Identifiable, Queryable, Selectable)]
#[diesel(table_name = tag_category)]
#[diesel(check_for_backend(Pg))]
pub struct TagCategory {
    pub id: i64,
    pub order: i32,
    pub name: SmallString,
    pub color: SmallString,
    pub last_edit_time: DateTime,
}

impl TagCategory {
    pub fn default() -> SqlLiteral<Bool, UncheckedBind<SqlLiteral<Bool>, tag_category::id>> {
        sql("").bind(tag_category::id).sql(" = 0")
    }
}
