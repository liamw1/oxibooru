use crate::api;
use crate::api::info::InfoResponse;
use crate::app::{AppState, Context};
use crate::config::Action;
use crate::extract::{Ctx, Json, Query, ResourceParams};
use crate::model::enums::{PostFlag, PostType};
use crate::resource::post::{Field, PostInfo};
use crate::time::BUILD_DATE;
use crate::web::Tab;
use crate::{time, unit};
use askama::Template;
use axum::response::Html;
use axum::{Router, routing};

pub fn routes() -> Router<AppState> {
    Router::new().route("/", routing::get(home))
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Template)]
#[template(path = "pages/home.html")]
struct HomeTemplate {
    ctx: Context,
    active_tab: Tab,
    info: InfoResponse,
}

async fn home(ctx: Ctx) -> Html<String> {
    let fields = [
        Field::Id,
        Field::User,
        Field::CanvasWidth,
        Field::CanvasHeight,
        Field::Type,
        Field::MimeType,
        Field::Flags,
        Field::CreationTime,
        Field::ContentUrl,
        Field::ThumbnailUrl,
    ]
    .into();
    let resource_params = Query(ResourceParams { query: None, fields });
    let Json(info) = api::info::get(ctx.clone(), resource_params).await.unwrap();

    let Ctx(ctx, _) = ctx;
    let active_tab = Tab::Home;
    HomeTemplate { ctx, active_tab, info }.render().map(Html).unwrap()
}
