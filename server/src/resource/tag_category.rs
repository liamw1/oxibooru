use crate::model::tag::TagCategory;
use crate::schema::{tag_category, tag_category_statistics};
use crate::time::DateTime;
use diesel::prelude::*;
use serde::Serialize;

#[derive(Serialize)]
pub struct TagCategoryInfo {
    version: DateTime,
    name: String,
    color: String,
    usages: i32,
    order: i32,
    default: bool,
}

impl TagCategoryInfo {
    pub fn new(conn: &mut PgConnection, category: TagCategory) -> QueryResult<Self> {
        let usages = tag_category_statistics::table
            .find(category.id)
            .select(tag_category_statistics::usage_count)
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
        let tag_categories: Vec<(TagCategory, i32)> = tag_category::table
            .inner_join(tag_category_statistics::table)
            .select((TagCategory::as_select(), tag_category_statistics::usage_count))
            .order(tag_category::order)
            .load(conn)?;
        Ok(tag_categories
            .into_iter()
            .map(|(category, usages)| Self {
                version: category.last_edit_time,
                name: category.name,
                color: category.color,
                usages,
                order: category.order,
                default: category.id == 0,
            })
            .collect())
    }
}
