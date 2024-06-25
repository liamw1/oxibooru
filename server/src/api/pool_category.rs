use crate::api;
use crate::model::enums::UserRank;
use crate::model::pool::PoolCategory;
use crate::schema::pool;
use crate::schema::pool_category;
use crate::util::DateTime;
use diesel::dsl::count;
use diesel::prelude::*;
use serde::Serialize;
use std::convert::Infallible;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list_pool_categories = warp::get()
        .and(warp::path!("pool-categories"))
        .and(api::auth())
        .and_then(list_pool_categories_endpoint);

    list_pool_categories
}

#[derive(Serialize)]
struct PoolCategoryInfo {
    version: DateTime,
    name: String,
    color: String,
    usages: i64,
    default: bool,
}

#[derive(Serialize)]
struct PoolCategoryList {
    results: Vec<PoolCategoryInfo>,
}

async fn list_pool_categories_endpoint(auth_result: api::AuthenticationResult) -> Result<api::Reply, Infallible> {
    Ok(api::access_level(auth_result).and_then(list_pool_categories).into())
}

fn list_pool_categories(access_level: UserRank) -> Result<PoolCategoryList, api::Error> {
    api::verify_privilege(access_level, "pool_categories:list")?;

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
                version: category.last_edit_time,
                name: category.name,
                color: category.color,
                usages: usages.unwrap_or(0),
                default: category.id == 0,
            })
            .collect(),
    })
}
