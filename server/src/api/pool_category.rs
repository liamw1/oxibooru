use crate::api;
use crate::api::ApiError;
use crate::api::AuthenticationResult;
use crate::api::Reply;
use crate::model::pool::PoolCategory;
use crate::model::rank::UserRank;
use crate::schema::pool;
use crate::schema::pool_category;
use diesel::dsl::count;
use diesel::prelude::*;
use serde::Serialize;
use warp::reject::Rejection;

pub async fn list_pool_categories(auth_result: AuthenticationResult) -> Result<Reply, Rejection> {
    Ok(Reply::from(api::access_level(auth_result).and_then(read_pool_categories)))
}

#[derive(Serialize)]
struct PoolCategoryInfo {
    version: i32,
    name: String,
    color: String,
    usages: i64,
    default: bool,
}

#[derive(Serialize)]
struct PoolCategoryList {
    results: Vec<PoolCategoryInfo>,
}

fn read_pool_categories(access_level: UserRank) -> Result<PoolCategoryList, ApiError> {
    if !access_level.has_permission_to("pool_categories:list") {
        return Err(ApiError::InsufficientPrivileges);
    }

    let mut conn = crate::establish_connection()?;
    let pool_categories = pool_category::table.select(PoolCategory::as_select()).load(&mut conn)?;
    let pool_category_usages: Vec<Option<i64>> = pool_category::table
        .left_join(pool::table.on(pool::category_id.eq(pool_category::id)))
        .select(count(pool::id).nullable())
        .group_by(pool_category::id)
        .order(pool_category::name.asc())
        .load(&mut conn)?;

    Ok(PoolCategoryList {
        results: pool_categories
            .into_iter()
            .zip(pool_category_usages.into_iter())
            .map(|(category, usages)| PoolCategoryInfo {
                version: 0,
                name: category.name,
                color: category.color,
                usages: usages.unwrap_or(0),
                default: category.id == 0,
            })
            .collect(),
    })
}
