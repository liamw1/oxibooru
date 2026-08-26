use crate::app::{AppState, Context};
use crate::config::{Action, RegexType};
use crate::extract::{Ctx, Json, Offset, Path, Query, ResourceParams};
use crate::resource::tag::{Field, TagInfo};
use crate::resource::tag_category::TagCategoryInfo;
use crate::string::SmallString;
use crate::web::pager::{Page, Pager};
use crate::web::{Html, Tab, WebError, WebResult};
use crate::{api, time, web};
use askama::Template;
use axum::{Router, routing};
use serde::{Deserialize, Serialize};
use server_macros::Deref;
use std::num::NonZeroU64;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/tags", routing::get(list))
        .route("/tag/{name}", routing::get(summary_tab))
        .route("/tag/{name}/edit", routing::get(edit_tab))
        .route("/tag/{name}/merge", routing::get(merge_tab))
        .route("/tag/{name}/delete", routing::get(delete_tab))
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
#[template(path = "pages/tags_page.html")]
struct ListTemplate<'a> {
    ctx: Context,
    active_tab: Tab,
    tags: Vec<TagInfo>,
    categories: Vec<TagCategoryInfo>,
    pager: Pager<'a, Params>,
    params: &'a Params,
}

async fn list(ctx: Ctx, Query(params): Query<Params>, Query(offset): Query<Offset>) -> WebResult<Html> {
    let fields = [
        Field::CreationTime,
        Field::Category,
        Field::Names,
        Field::Implications,
        Field::Suggestions,
        Field::Usages,
    ]
    .into();

    let query = params.search_text.clone();
    let resource_params = Query(ResourceParams { query, fields });
    let page_params = Query(offset.to_page_params(LIMIT));
    let Json(response) = api::tag::list(ctx.clone(), resource_params, page_params).await?;
    let categories = web::tag_category::get_categories(ctx.clone()).await?;

    let params = params.simplify();
    let pager = Pager::build("tags", &params, page_params, response.total);

    let Ctx(ctx, _) = ctx;
    ListTemplate {
        ctx,
        active_tab: Tab::Tag,
        tags: response.results,
        categories,
        pager,
        params: &params,
    }
    .render()
    .map(Html)
    .map_err(WebError::from)
}

#[derive(PartialEq, Eq)]
enum TagTab {
    Summary,
    Edit,
    Merge,
    Delete,
}

struct FullTagPage {
    ctx: Context,
    active_tab: Tab,
    active_tag_tab: TagTab,
    tag: TagInfo,
    categories: Vec<TagCategoryInfo>,
}

async fn view(ctx: Ctx, path: Path<SmallString>, active_tag_tab: TagTab) -> WebResult<FullTagPage> {
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
    let Json(tag) = api::tag::get(ctx.clone(), path, resource_params).await?;
    let categories = web::tag_category::get_categories(ctx.clone()).await?;

    let Ctx(ctx, _) = ctx;
    Ok(FullTagPage {
        ctx,
        active_tab: Tab::Tag,
        active_tag_tab,
        tag,
        categories,
    })
}

#[derive(Deref, Template)]
#[template(path = "pages/tag_summary.html")]
struct SummaryPageTemplate(FullTagPage);

#[derive(Template)]
#[template(path = "pages/tag_summary.html", block = "tag")]
struct SummaryFragmentTemplate {
    active_tag_tab: TagTab,
    tag: TagInfo,
}

async fn summary_tab(ctx: Ctx, path: Path<SmallString>) -> WebResult<Html> {
    let page_info = view(ctx, path, TagTab::Summary).await?;
    SummaryPageTemplate(page_info)
        .render()
        .map(Html)
        .map_err(WebError::from)
}

#[derive(Deref, Template)]
#[template(path = "pages/tag_edit.html")]
struct EditPageTemplate(FullTagPage);

async fn edit_tab(ctx: Ctx, path: Path<SmallString>) -> WebResult<Html> {
    let page_info = view(ctx, path, TagTab::Edit).await?;
    EditPageTemplate(page_info).render().map(Html).map_err(WebError::from)
}

#[derive(Deref, Template)]
#[template(path = "pages/tag_merge.html")]
struct MergePageTemplate(FullTagPage);

async fn merge_tab(ctx: Ctx, path: Path<SmallString>) -> WebResult<Html> {
    let page_info = view(ctx, path, TagTab::Merge).await?;
    MergePageTemplate(page_info).render().map(Html).map_err(WebError::from)
}

#[derive(Deref, Template)]
#[template(path = "pages/tag_delete.html")]
struct DeletePageTemplate(FullTagPage);

async fn delete_tab(ctx: Ctx, path: Path<SmallString>) -> WebResult<Html> {
    let page_info = view(ctx, path, TagTab::Delete).await?;
    DeletePageTemplate(page_info).render().map(Html).map_err(WebError::from)
}
