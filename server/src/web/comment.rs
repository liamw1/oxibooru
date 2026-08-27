use crate::app::AppState;
use crate::config::Action;
use crate::extract::{Ctx, Json, Offset, Query, ResourceParams};
use crate::resource::post::{Field, PostInfo};
use crate::web::pager::{Page, Pager};
use crate::web::{Html, Tab, WebError, WebResult};
use crate::{api, time};
use askama::Template;
use axum::{Router, routing};
use std::num::NonZeroU64;

pub fn routes() -> Router<AppState> {
    Router::new().route("/comments", routing::get(list))
}

const LIMIT: NonZeroU64 = NonZeroU64::new(10).unwrap();

#[derive(Template)]
#[template(path = "pages/comment/list.html")]
struct ListTemplate<'a> {
    ctx: Ctx,
    active_tab: Tab,
    posts: Vec<PostInfo>,
    pager: Pager<'a, ()>,
}

async fn list(ctx: Ctx, Query(offset): Query<Offset>) -> WebResult<Html> {
    let fields = [Field::Id, Field::ThumbnailUrl, Field::Comments].into();

    let query = Some("sort:comment-date comment-count:1..".to_owned());
    let resource_params = Query(ResourceParams { query, fields });
    let page_params = Query(offset.to_page_params(LIMIT));
    let Json(response) = api::post::list(ctx.clone(), resource_params, page_params).await?;

    let pager = Pager::build("comments", &(), page_params, response.total);
    ListTemplate {
        ctx,
        active_tab: Tab::Comment,
        posts: response.results,
        pager,
    }
    .render()
    .map(Html)
    .map_err(WebError::from)
}
