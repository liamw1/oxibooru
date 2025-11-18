use crate::schema::pool_category;
use crate::string::SmallString;
use crate::time::DateTime;
use diesel::dsl::sql;
use diesel::expression::{SqlLiteral, UncheckedBind};
use diesel::pg::Pg;
use diesel::sql_types::Bool;
use diesel::{AsChangeset, Identifiable, Insertable, Queryable, Selectable};

#[derive(Insertable)]
#[diesel(table_name = pool_category)]
#[diesel(check_for_backend(Pg))]
pub struct NewPoolCategory<'a> {
    pub name: &'a str,
    pub color: &'a str,
}

#[derive(Clone, AsChangeset, Identifiable, Queryable, Selectable)]
#[diesel(table_name = pool_category)]
#[diesel(check_for_backend(Pg))]
pub struct PoolCategory {
    pub id: i64,
    pub name: SmallString,
    pub color: SmallString,
    pub last_edit_time: DateTime,
}

impl PoolCategory {
    pub fn default() -> SqlLiteral<Bool, UncheckedBind<SqlLiteral<Bool>, pool_category::id>> {
        sql("").bind(pool_category::id).sql(" = 0")
    }
}
