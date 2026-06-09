use crate::app::{AppState, Context};
use crate::config::Action;
use crate::extract::{Ctx, Json, PageParams, Path, Query, ResourceParams};
use crate::resource::tag::{Field, TagInfo};
use crate::string::SmallString;
use crate::web::Tab;
use crate::web::pager::{Page, Pager};
use crate::{api, time};
use askama::Template;
use axum::response::Html;
use axum::{Router, routing};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/tags", routing::get(list))
        .route("/tag/{name}", routing::get(summary))
        .route("/tag/{name}/edit", routing::get(edit))
        .route("/tag/{name}/merge", routing::get(merge))
        .route("/tag/{name}/delete", routing::get(delete))
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
#[template(path = "pages/tags_page.html")]
struct ListTemplate<'a> {
    ctx: Context,
    active_tab: Tab,
    tags: Vec<TagInfo>,
    pager: Pager<'a, Params>,
    params: &'a Params,
}

async fn list(ctx: Ctx, Query(params): Query<Params>, page_params: Query<PageParams>) -> Html<String> {
    let fields = [
        Field::CreationTime,
        Field::Names,
        Field::Implications,
        Field::Suggestions,
        Field::Usages,
    ]
    .into();

    let query = params.search_text.clone();
    let resource_params = Query(ResourceParams { query, fields });
    let Json(response) = api::tag::list(ctx.clone(), resource_params, page_params).await.unwrap();

    let params = params.simplify();
    let pager = Pager::build("tags", &params, page_params, response.total);

    let Ctx(ctx, _) = ctx;
    ListTemplate {
        ctx,
        active_tab: Tab::Tag,
        tags: response.results,
        pager,
        params: &params,
    }
    .render()
    .map(Html)
    .unwrap()
}

#[derive(PartialEq, Eq)]
enum TagTab {
    Summary,
    Edit,
    Merge,
    Delete,
}

#[derive(Template)]
#[template(path = "pages/tag.html")]
struct TagTemplate {
    ctx: Context,
    active_tab: Tab,
    active_tag_tab: TagTab,
    tag: TagInfo,
}

async fn view(ctx: Ctx, path: Path<SmallString>, active_tag_tab: TagTab) -> Html<String> {
    let fields = [
        Field::Description,
        Field::Category,
        Field::Names,
        Field::Implications,
        Field::Suggestions,
        Field::Usages,
    ]
    .into();

    let resource_params = Query(ResourceParams { query: None, fields });
    let Json(tag) = api::tag::get(ctx.clone(), path, resource_params).await.unwrap();

    let Ctx(ctx, _) = ctx;
    TagTemplate {
        ctx,
        active_tab: Tab::Tag,
        active_tag_tab,
        tag,
    }
    .render()
    .map(Html)
    .unwrap()
}

async fn summary(ctx: Ctx, path: Path<SmallString>) -> Html<String> {
    view(ctx, path, TagTab::Summary).await
}

async fn edit(ctx: Ctx, path: Path<SmallString>) -> Html<String> {
    view(ctx, path, TagTab::Edit).await
}

async fn merge(ctx: Ctx, path: Path<SmallString>) -> Html<String> {
    view(ctx, path, TagTab::Merge).await
}

async fn delete(ctx: Ctx, path: Path<SmallString>) -> Html<String> {
    view(ctx, path, TagTab::Delete).await
}
