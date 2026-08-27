use crate::app::AppState;
use crate::config::Action;
use crate::extract::{Ctx, Json, Offset, Path, Query, ResourceParams};
use crate::resource::pool::{Field, PoolInfo};
use crate::resource::pool_category::PoolCategoryInfo;
use crate::web::pager::{Page, Pager};
use crate::web::{Html, Tab, WebError, WebResult};
use crate::{api, time, web};
use askama::Template;
use axum::{Router, routing};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/pools", routing::get(list))
        .route("/pool/{id}", routing::get(summary_tab))
        .route("/pool/{id}/edit", routing::get(edit_tab))
        .route("/pool/{id}/merge", routing::get(merge_tab))
        .route("/pool/{id}/delete", routing::get(delete_tab))
}

const LIMIT: NonZeroU64 = NonZeroU64::new(50).unwrap();

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Params {
    search_text: Option<String>,
}

impl Params {
    fn search_text(&self) -> &str {
        self.search_text.as_deref().unwrap_or("")
    }

    fn simplify(mut self) -> Self {
        if self.search_text.as_ref().is_some_and(String::is_empty) {
            self.search_text = None;
        }
        self
    }
}

#[derive(Template)]
#[template(path = "pages/pool/list.html")]
struct ListTemplate<'a> {
    ctx: Ctx,
    active_tab: Tab,
    pools: Vec<PoolInfo>,
    categories: Vec<PoolCategoryInfo>,
    pager: Pager<'a, Params>,
    params: &'a Params,
}

async fn list(ctx: Ctx, Query(params): Query<Params>, Query(offset): Query<Offset>) -> WebResult<Html> {
    let fields = [
        Field::Id,
        Field::CreationTime,
        Field::Category,
        Field::Names,
        Field::PostCount,
    ]
    .into();

    let query = params.search_text.clone();
    let resource_params = Query(ResourceParams { query, fields });
    let page_params = Query(offset.to_page_params(LIMIT));
    let Json(response) = api::pool::list(ctx.clone(), resource_params, page_params).await?;
    let categories = web::pool_category::get_categories(ctx.clone()).await?;

    let params = params.simplify();
    let pager = Pager::build("pools", &params, page_params, response.total);
    ListTemplate {
        ctx,
        active_tab: Tab::Pool,
        pools: response.results,
        categories,
        pager,
        params: &params,
    }
    .render()
    .map(Html)
    .map_err(WebError::from)
}

#[derive(PartialEq, Eq)]
enum PoolTab {
    Summary,
    Edit,
    Merge,
    Delete,
}

#[derive(Template)]
#[template(path = "pages/pool/base.html")]
struct PoolTemplate {
    ctx: Ctx,
    active_tab: Tab,
    active_pool_tab: PoolTab,
    pool: PoolInfo,
    categories: Vec<PoolCategoryInfo>,
}

async fn view(ctx: Ctx, path: Path<i64>, active_pool_tab: PoolTab) -> WebResult<Html> {
    let fields = [
        Field::Id,
        Field::Description,
        Field::Category,
        Field::Names,
        Field::PostCount,
    ]
    .into();

    let resource_params = Query(ResourceParams { query: None, fields });
    let Json(pool) = api::pool::get(ctx.clone(), path, resource_params).await?;
    let categories = web::pool_category::get_categories(ctx.clone()).await?;
    PoolTemplate {
        ctx,
        active_tab: Tab::Pool,
        active_pool_tab,
        pool,
        categories,
    }
    .render()
    .map(Html)
    .map_err(WebError::from)
}

async fn summary_tab(ctx: Ctx, path: Path<i64>) -> WebResult<Html> {
    view(ctx, path, PoolTab::Summary).await
}

async fn edit_tab(ctx: Ctx, path: Path<i64>) -> WebResult<Html> {
    view(ctx, path, PoolTab::Edit).await
}

async fn merge_tab(ctx: Ctx, path: Path<i64>) -> WebResult<Html> {
    view(ctx, path, PoolTab::Merge).await
}

async fn delete_tab(ctx: Ctx, path: Path<i64>) -> WebResult<Html> {
    view(ctx, path, PoolTab::Delete).await
}
