use crate::app::{AppState, Context};
use crate::config::Action;
use crate::extract::{Ctx, Json, Offset, Query, ResourceParams};
use crate::resource::snapshot::{Field, SnapshotInfo};
use crate::web::pager::{Page, Pager};
use crate::web::{Html, Tab, WebError, WebResult};
use crate::{api, time};
use askama::Template;
use axum::{Router, routing};
use std::num::NonZeroU64;

pub fn routes() -> Router<AppState> {
    Router::new().route("/history", routing::get(history))
}

const LIMIT: NonZeroU64 = NonZeroU64::new(25).unwrap();

#[derive(Template)]
#[template(path = "pages/snapshots_page.html")]
struct ListTemplate<'a> {
    ctx: Context,
    active_tab: Tab,
    snapshots: Vec<SnapshotInfo>,
    pager: Pager<'a, ()>,
}

async fn history(ctx: Ctx, Query(offset): Query<Offset>) -> WebResult<Html> {
    let fields = [Field::User, Field::Operation, Field::Time].into();

    let resource_params = Query(ResourceParams { query: None, fields });
    let page_params = Query(offset.to_page_params(LIMIT));
    let Json(response) = api::snapshot::list(ctx.clone(), resource_params, page_params).await?;

    let pager = Pager::build("history", &(), page_params, response.total);

    let Ctx(ctx, _) = ctx;
    ListTemplate {
        ctx,
        active_tab: Tab::None,
        snapshots: response.results,
        pager,
    }
    .render()
    .map(Html)
    .map_err(WebError::from)
}
