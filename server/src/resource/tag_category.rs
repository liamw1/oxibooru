use crate::model::tag::TagCategory;
use crate::schema::{tag_category, tag_category_statistics};
use crate::time::DateTime;
use diesel::prelude::*;
use serde::Serialize;
use serde_with::skip_serializing_none;
use std::str::FromStr;
use strum::{EnumString, EnumTable};

#[derive(Clone, Copy, EnumString, EnumTable)]
#[strum(serialize_all = "camelCase")]
pub enum Field {
    Version,
    Name,
    Color,
    Usages,
    Order,
    Default,
}

impl Field {
    pub fn create_table(fields: Option<&str>) -> Result<FieldTable<bool>, <Self as FromStr>::Err> {
        if let Some(fields_str) = fields {
            let mut table = FieldTable::filled(false);
            for field in fields_str.split(',') {
                table[Self::from_str(field)?] = true;
            }
            Ok(table)
        } else {
            Ok(FieldTable::filled(true))
        }
    }
}

#[skip_serializing_none]
#[derive(Serialize)]
pub struct TagCategoryInfo {
    version: Option<DateTime>,
    name: Option<String>,
    color: Option<String>,
    usages: Option<i64>,
    order: Option<i32>,
    default: Option<bool>,
}

impl TagCategoryInfo {
    pub fn new(conn: &mut PgConnection, category: TagCategory, fields: &FieldTable<bool>) -> QueryResult<Self> {
        let usages = tag_category_statistics::table
            .find(category.id)
            .select(tag_category_statistics::usage_count)
            .first(conn)?;
        Ok(Self {
            version: fields[Field::Version].then_some(category.last_edit_time),
            name: fields[Field::Name].then_some(category.name),
            color: fields[Field::Color].then_some(category.color),
            usages: fields[Field::Usages].then_some(usages),
            order: fields[Field::Order].then_some(category.order),
            default: fields[Field::Default].then_some(category.id == 0),
        })
    }

    pub fn new_from_id(conn: &mut PgConnection, category_id: i64, fields: &FieldTable<bool>) -> QueryResult<Self> {
        let category = tag_category::table.find(category_id).first(conn)?;
        Self::new(conn, category, fields)
    }

    pub fn all(conn: &mut PgConnection, fields: &FieldTable<bool>) -> QueryResult<Vec<Self>> {
        let tag_categories: Vec<(TagCategory, i64)> = tag_category::table
            .inner_join(tag_category_statistics::table)
            .select((TagCategory::as_select(), tag_category_statistics::usage_count))
            .order(tag_category::order)
            .load(conn)?;
        Ok(tag_categories
            .into_iter()
            .map(|(category, usages)| Self {
                version: fields[Field::Version].then_some(category.last_edit_time),
                name: fields[Field::Name].then_some(category.name),
                color: fields[Field::Color].then_some(category.color),
                usages: fields[Field::Usages].then_some(usages),
                order: fields[Field::Order].then_some(category.order),
                default: fields[Field::Default].then_some(category.id == 0),
            })
            .collect())
    }
}
