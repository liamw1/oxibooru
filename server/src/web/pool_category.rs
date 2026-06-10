use crate::api;
use crate::api::error::ApiResult;
use crate::extract::{Ctx, Json, Query, ResourceParams};
use crate::resource::pool_category::{Field, PoolCategoryInfo};

pub async fn get_categories(ctx: Ctx) -> ApiResult<Vec<PoolCategoryInfo>> {
    let fields = [Field::Name, Field::Color].into();
    let resource_params = Query(ResourceParams { query: None, fields });
    api::pool_category::list(ctx, resource_params)
        .await
        .map(|Json(response)| response.results)
}
