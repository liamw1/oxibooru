use crate::schema::pool_category;
use crate::string::SmallString;
use crate::time::DateTime;
use diesel::pg::Pg;
use diesel::prelude::*;

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
