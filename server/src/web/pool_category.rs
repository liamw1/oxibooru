use crate::api;
use crate::api::error::ApiResult;
use crate::app::AppState;
use crate::config::Action;
use crate::extract::{Ctx, Json, Query, ResourceParams};
use crate::resource::pool_category::{Field, PoolCategoryInfo};
use crate::web::{Html, Tab, WebError, WebResult};
use askama::Template;
use axum::{Router, routing};

pub fn routes() -> Router<AppState> {
    Router::new().route("/pool-categories", routing::get(list))
}

pub async fn get_categories(ctx: Ctx) -> ApiResult<Vec<PoolCategoryInfo>> {
    let fields = [Field::Name, Field::Color].into();
    let resource_params = Query(ResourceParams { query: None, fields });
    api::pool_category::list(ctx, resource_params)
        .await
        .map(|Json(response)| response.results)
}

#[derive(Template)]
#[template(path = "pages/pool_categories.html")]
struct ListTemplate {
    ctx: Ctx,
    active_tab: Tab,
    categories: Vec<PoolCategoryInfo>,
}

async fn list(ctx: Ctx) -> WebResult<Html> {
    let fields = [Field::Name, Field::Color, Field::Usages, Field::Default].into();

    let resource_params = Query(ResourceParams { query: None, fields });
    let Json(response) = api::pool_category::list(ctx.clone(), resource_params).await?;

    ListTemplate {
        ctx,
        active_tab: Tab::Pool,
        categories: response.results,
    }
    .render()
    .map(Html)
    .map_err(WebError::from)
}
