use crate::api::ApiError;
use crate::model::rank::UserRank;
use crate::model::tag::TagCategory;
use crate::schema::tag;
use crate::schema::tag_category;
use diesel::dsl::count;
use diesel::prelude::*;
use serde::Serialize;
use warp::reject::Rejection;
use warp::reply::Reply;

pub async fn list_tag_categories(privilege: UserRank) -> Result<Box<dyn Reply>, Rejection> {
    Ok(match collect_tag_categories(privilege) {
        Ok(categories) => Box::new(warp::reply::json(&categories)),
        Err(err) => Box::new(err.to_reply()),
    })
}

#[derive(Serialize)]
struct TagCategoryInfo {
    version: i32,
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

fn collect_tag_categories(privilege: UserRank) -> Result<TagCategoryList, ApiError> {
    if !privilege.has_permission_to("tag_categories:list") {
        return Err(ApiError::InsufficientPrivileges);
    }

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
                version: 0,
                name: category.name,
                color: category.color,
                usages: usages.unwrap_or(0),
                order: category.order,
                default: category.id == 0,
            })
            .collect(),
    })
}
