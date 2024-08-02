use crate::model::tag::TagCategory;
use crate::schema::{tag, tag_category};
use crate::util::DateTime;
use diesel::dsl;
use diesel::prelude::*;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize)]
pub struct TagCategoryInfo {
    version: DateTime,
    name: String,
    color: String,
    usages: i64,
    order: i32,
    default: bool,
}

impl TagCategoryInfo {
    pub fn new(conn: &mut PgConnection, category: TagCategory) -> QueryResult<Self> {
        let usages = tag::table
            .filter(tag::category_id.eq(category.id))
            .count()
            .first(conn)?;

        Ok(Self {
            version: category.last_edit_time,
            name: category.name,
            color: category.color,
            usages,
            order: category.order,
            default: category.id == 0,
        })
    }

    pub fn new_from_id(conn: &mut PgConnection, category_id: i32) -> QueryResult<Self> {
        let category = tag_category::table.find(category_id).first(conn)?;
        Self::new(conn, category)
    }

    pub fn all(conn: &mut PgConnection) -> QueryResult<Vec<Self>> {
        let tag_categories: Vec<TagCategory> = tag_category::table.order(tag_category::order).load(conn)?;
        let tag_category_usages: HashMap<i32, Option<i64>> = tag_category::table
            .left_join(tag::table)
            .group_by(tag_category::id)
            .select((tag_category::id, dsl::count(tag::id).nullable()))
            .load(conn)?
            .into_iter()
            .collect();

        Ok(tag_categories
            .into_iter()
            .map(|category| Self {
                version: category.last_edit_time,
                name: category.name,
                color: category.color,
                usages: tag_category_usages[&category.id].unwrap_or(0),
                order: category.order,
                default: category.id == 0,
            })
            .collect())
    }
}
