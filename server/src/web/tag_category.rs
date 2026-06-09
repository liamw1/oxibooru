use crate::api;
use crate::api::error::ApiResult;
use crate::extract::{Ctx, Json, Query, ResourceParams};
use crate::resource::tag_category::{Field, TagCategoryInfo};

pub async fn get_categories(ctx: Ctx) -> ApiResult<Vec<TagCategoryInfo>> {
    let fields = [Field::Name, Field::Color].into();
    let resource_params = Query(ResourceParams { query: None, fields });
    api::tag_category::list(ctx, resource_params)
        .await
        .map(|Json(response)| response.results)
}
