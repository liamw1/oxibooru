use crate::api::info;
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

struct ServerInfo {
    post_count: i64,
    disk_usage: String,
    featured_post: Option<PostInfo>,
    time_since_build: String,
}

#[derive(Template)]
#[template(path = "pages/home.html")]
struct HomeTemplate {
    ctx: Context,
    info: ServerInfo,
    active_tab: Tab,
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
    let Json(response) = info::get(ctx.clone(), resource_params).await.unwrap();

    let info = ServerInfo {
        post_count: response.post_count,
        disk_usage: unit::format_bytes(u64::try_from(response.disk_usage).unwrap_or(0)),
        featured_post: response.featured_post,
        time_since_build: time::since(BUILD_DATE),
    };

    let Ctx(ctx, _) = ctx;
    let active_tab = Tab::Home;
    HomeTemplate { ctx, info, active_tab }.render().map(Html).unwrap()
}
