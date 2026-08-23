use crate::app::{AppState, Context};
use crate::config::Action;
use crate::extract::{Ctx, Json, PageParams, Query, ResourceParams};
use crate::resource::post::{Field, PostInfo};
use crate::web::Tab;
use crate::web::pager::{Page, Pager};
use crate::{api, time};
use askama::Template;
use axum::response::Html;
use axum::{Router, routing};

pub fn routes() -> Router<AppState> {
    Router::new().route("/comments", routing::get(list))
}

#[derive(Template)]
#[template(path = "pages/comments_page.html")]
struct ListTemplate<'a> {
    ctx: Context,
    active_tab: Tab,
    posts: Vec<PostInfo>,
    pager: Pager<'a, ()>,
}

async fn list(ctx: Ctx, page_params: Query<PageParams>) -> Html<String> {
    let fields = [Field::Id, Field::ThumbnailUrl, Field::Comments].into();

    let query = Some("sort:comment-date comment-count:1..".to_owned());
    let resource_params = Query(ResourceParams { query, fields });
    let Json(response) = api::post::list(ctx.clone(), resource_params, page_params)
        .await
        .unwrap();

    let pager = Pager::build("comments", &(), page_params, response.total);

    let Ctx(ctx, _) = ctx;
    ListTemplate {
        ctx,
        active_tab: Tab::Comment,
        posts: response.results,
        pager,
    }
    .render()
    .map(Html)
    .unwrap()
}
