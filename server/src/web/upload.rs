use crate::app::AppState;
use crate::config::Action;
use crate::extract::Ctx;
use crate::web::{Html, Tab, WebError, WebResult};
use askama::Template;
use axum::{Router, routing};

pub fn routes() -> Router<AppState> {
    Router::new().route("/upload", routing::get(upload))
}

#[derive(Template)]
#[template(path = "pages/upload.html")]
struct UploadTemplate {
    ctx: Ctx,
    active_tab: Tab,
}

async fn upload(ctx: Ctx) -> WebResult<Html> {
    UploadTemplate {
        ctx,
        active_tab: Tab::Upload,
    }
    .render()
    .map(Html)
    .map_err(WebError::from)
}
