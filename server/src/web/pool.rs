use crate::app::{AppState, Context};
use crate::config::Action;
use crate::extract::{Ctx, Json, PageParams, Path, Query, ResourceParams};
use crate::resource::pool::{Field, PoolInfo};
use crate::resource::pool_category::PoolCategoryInfo;
use crate::web::Tab;
use crate::web::pager::{Page, Pager};
use crate::{api, time, web};
use askama::Template;
use axum::response::Html;
use axum::{Router, routing};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/pools", routing::get(list))
        .route("/pool/{id}", routing::get(summary))
        .route("/pool/{id}/edit", routing::get(edit))
        .route("/pool/{id}/merge", routing::get(merge))
        .route("/pool/{id}/delete", routing::get(delete))
}

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
#[template(path = "pages/pools_page.html")]
struct ListTemplate<'a> {
    ctx: Context,
    active_tab: Tab,
    pools: Vec<PoolInfo>,
    categories: Vec<PoolCategoryInfo>,
    pager: Pager<'a, Params>,
    params: &'a Params,
}

async fn list(ctx: Ctx, Query(params): Query<Params>, page_params: Query<PageParams>) -> Html<String> {
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
    let Json(response) = api::pool::list(ctx.clone(), resource_params, page_params)
        .await
        .unwrap();
    let categories = web::pool_category::get_categories(ctx.clone()).await.unwrap();

    let params = params.simplify();
    let pager = Pager::build("pools", &params, page_params, response.total);

    let Ctx(ctx, _) = ctx;
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
    .unwrap()
}

#[derive(PartialEq, Eq)]
enum PoolTab {
    Summary,
    Edit,
    Merge,
    Delete,
}

#[derive(Template)]
#[template(path = "pages/pool.html")]
struct TagTemplate {
    ctx: Context,
    active_tab: Tab,
    active_pool_tab: PoolTab,
    pool: PoolInfo,
    categories: Vec<PoolCategoryInfo>,
}

async fn view(ctx: Ctx, path: Path<i64>, active_pool_tab: PoolTab) -> Html<String> {
    let fields = [
        Field::Id,
        Field::Description,
        Field::Category,
        Field::Names,
        Field::PostCount,
    ]
    .into();

    let resource_params = Query(ResourceParams { query: None, fields });
    let Json(pool) = api::pool::get(ctx.clone(), path, resource_params).await.unwrap();
    let categories = web::pool_category::get_categories(ctx.clone()).await.unwrap();

    let Ctx(ctx, _) = ctx;
    TagTemplate {
        ctx,
        active_tab: Tab::Tag,
        active_pool_tab,
        pool,
        categories,
    }
    .render()
    .map(Html)
    .unwrap()
}

async fn summary(ctx: Ctx, path: Path<i64>) -> Html<String> {
    view(ctx, path, PoolTab::Summary).await
}

async fn edit(ctx: Ctx, path: Path<i64>) -> Html<String> {
    view(ctx, path, PoolTab::Edit).await
}

async fn merge(ctx: Ctx, path: Path<i64>) -> Html<String> {
    view(ctx, path, PoolTab::Merge).await
}

async fn delete(ctx: Ctx, path: Path<i64>) -> Html<String> {
    view(ctx, path, PoolTab::Delete).await
}
