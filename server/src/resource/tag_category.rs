use crate::model::tag::TagCategory;
use crate::schema::{tag, tag_category};
use crate::time::DateTime;
use diesel::dsl::count;
use diesel::prelude::*;
use serde::Serialize;

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
        let tag_categories: Vec<(TagCategory, Option<i64>)> = tag_category::table
            .left_join(tag::table)
            .group_by(tag_category::id)
            .select((TagCategory::as_select(), count(tag::id).nullable()))
            .load(conn)?;

        Ok(tag_categories
            .into_iter()
            .map(|(category, usages)| Self {
                version: category.last_edit_time,
                name: category.name,
                color: category.color,
                usages: usages.unwrap_or(0),
                order: category.order,
                default: category.id == 0,
            })
            .collect())
    }
}
