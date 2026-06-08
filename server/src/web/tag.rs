use crate::app::{AppState, Context};
use crate::config::Action;
use crate::extract::{Ctx, Json, PageParams, Query, ResourceParams};
use crate::resource::tag::{Field, TagInfo};
use crate::web::Tab;
use crate::web::pager::{Page, Pager};
use crate::{api, time};
use askama::Template;
use axum::response::Html;
use axum::{Router, routing};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new().route("/tags", routing::get(list))
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
