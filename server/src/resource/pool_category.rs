use crate::model::pool::PoolCategory;
use crate::schema::{pool_category, pool_category_statistics};
use crate::time::DateTime;
use diesel::prelude::*;
use serde::Serialize;

#[derive(Serialize)]
pub struct PoolCategoryInfo {
    version: DateTime,
    name: String,
    color: String,
    usages: i32,
    default: bool,
}

impl PoolCategoryInfo {
    pub fn new(conn: &mut PgConnection, category: PoolCategory) -> QueryResult<Self> {
        let usages = pool_category_statistics::table
            .find(category.id)
            .select(pool_category_statistics::usage_count)
            .first(conn)?;
        Ok(Self {
            version: category.last_edit_time,
            name: category.name,
            color: category.color,
            usages,
            default: category.id == 0,
        })
    }

    pub fn new_from_id(conn: &mut PgConnection, category_id: i32) -> QueryResult<Self> {
        let category = pool_category::table.find(category_id).first(conn)?;
        Self::new(conn, category)
    }

    pub fn all(conn: &mut PgConnection) -> QueryResult<Vec<Self>> {
        let pool_categories: Vec<(PoolCategory, i32)> = pool_category::table
            .inner_join(pool_category_statistics::table)
            .select((PoolCategory::as_select(), pool_category_statistics::usage_count))
            .load(conn)?;
        Ok(pool_categories
            .into_iter()
            .map(|(category, usages)| Self {
                version: category.last_edit_time,
                name: category.name,
                color: category.color,
                usages,
                default: category.id == 0,
            })
            .collect())
    }
}
