use crate::api;
use crate::model::enums::UserRank;
use crate::model::tag::TagCategory;
use crate::schema::tag;
use crate::schema::tag_category;
use crate::util::DateTime;
use diesel::dsl::count;
use diesel::prelude::*;
use serde::Serialize;
use warp::reject::Rejection;

pub async fn list_tag_categories(auth_result: api::AuthenticationResult) -> Result<api::Reply, Rejection> {
    Ok(api::access_level(auth_result).and_then(read_tag_categories).into())
}

#[derive(Serialize)]
struct TagCategoryInfo {
    version: DateTime,
    name: String,
    color: String,
    usages: i64,
    order: i32,
    default: bool,
}

#[derive(Serialize)]
struct TagCategoryList {
    results: Vec<TagCategoryInfo>,
}

fn read_tag_categories(access_level: UserRank) -> Result<TagCategoryList, api::Error> {
    api::validate_privilege(access_level, "tag_categories:list")?;

    let mut conn = crate::establish_connection()?;
    let tag_categories = tag_category::table.select(TagCategory::as_select()).load(&mut conn)?;
    let tag_category_usages: Vec<Option<i64>> = tag_category::table
        .left_join(tag::table.on(tag::category_id.eq(tag_category::id)))
        .select(count(tag::id).nullable())
        .group_by(tag_category::id)
        .order(tag_category::order.asc())
        .load(&mut conn)?;

    Ok(TagCategoryList {
        results: tag_categories
            .into_iter()
            .zip(tag_category_usages.into_iter())
            .map(|(category, usages)| TagCategoryInfo {
                version: category.last_edit_time,
                name: category.name,
                color: category.color,
                usages: usages.unwrap_or(0),
                order: category.order,
                default: category.id == 0,
            })
            .collect(),
    })
}
