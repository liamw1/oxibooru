use crate::model::pool::PoolCategory;
use crate::schema::{pool, pool_category};
use crate::time::DateTime;
use diesel::{dsl, prelude::*};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize)]
pub struct PoolCategoryInfo {
    version: DateTime,
    name: String,
    color: String,
    usages: i64,
    default: bool,
}

impl PoolCategoryInfo {
    pub fn new(conn: &mut PgConnection, category: PoolCategory) -> QueryResult<Self> {
        let usages = pool::table
            .filter(pool::category_id.eq(category.id))
            .count()
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
        let pool_categories: Vec<PoolCategory> = pool_category::table.load(conn)?;
        let pool_category_usages: HashMap<i32, Option<i64>> = pool_category::table
            .left_join(pool::table)
            .group_by(pool_category::id)
            .select((pool_category::id, dsl::count(pool::id).nullable()))
            .load(conn)?
            .into_iter()
            .collect();

        Ok(pool_categories
            .into_iter()
            .map(|category| Self {
                version: category.last_edit_time,
                name: category.name,
                color: category.color,
                usages: pool_category_usages[&category.id].unwrap_or(0),
                default: category.id == 0,
            })
            .collect())
    }
}
