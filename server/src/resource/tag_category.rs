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
    pub fn all(conn: &mut PgConnection) -> QueryResult<Vec<Self>> {
        let tag_categories = tag_category::table
            .select(TagCategory::as_select())
            .order_by(tag_category::order.asc())
            .load(conn)?;
        let tag_category_usages: HashMap<i32, i64> = tag_category::table
            .inner_join(tag::table)
            .group_by(tag_category::id)
            .select((tag_category::id, dsl::count(tag::id)))
            .load(conn)?
            .into_iter()
            .collect();

        Ok(tag_categories
            .into_iter()
            .map(|category| TagCategoryInfo {
                version: category.last_edit_time,
                name: category.name,
                color: category.color,
                usages: tag_category_usages[&category.id],
                order: category.order,
                default: category.id == 0,
            })
            .collect())
    }
}
