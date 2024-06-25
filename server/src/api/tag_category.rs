use crate::api;
use crate::model::enums::UserRank;
use crate::model::tag::TagCategory;
use crate::schema::tag;
use crate::schema::tag_category;
use crate::util::DateTime;
use diesel::dsl::count;
use diesel::prelude::*;
use serde::Serialize;
use std::convert::Infallible;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list_tag_categories = warp::get()
        .and(warp::path!("tag-categories"))
        .and(api::auth())
        .and_then(list_tag_categories_endpoint);

    list_tag_categories
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

async fn list_tag_categories_endpoint(auth_result: api::AuthenticationResult) -> Result<api::Reply, Infallible> {
    Ok(api::access_level(auth_result).and_then(list_tag_categories).into())
}

fn list_tag_categories(access_level: UserRank) -> Result<TagCategoryList, api::Error> {
    api::verify_privilege(access_level, "tag_categories:list")?;

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
