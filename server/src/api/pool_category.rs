use crate::api::{ApiResult, AuthResult};
use crate::resource::pool_category::PoolCategoryInfo;
use crate::{api, config};
use diesel::prelude::*;
use serde::Serialize;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list_pool_categories = warp::get()
        .and(warp::path!("pool-categories"))
        .and(api::auth())
        .map(list_pool_categories)
        .map(api::Reply::from);

    list_pool_categories
}

#[derive(Serialize)]
struct PoolCategoryList {
    results: Vec<PoolCategoryInfo>,
}

fn list_pool_categories(auth: AuthResult) -> ApiResult<PoolCategoryList> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().pool_category_list)?;

    crate::establish_connection()?.transaction(|conn| {
        PoolCategoryInfo::all(conn)
            .map(|results| PoolCategoryList { results })
            .map_err(api::Error::from)
    })
}
