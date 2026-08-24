use crate::app::{AppState, Context};
use crate::config::Action;
use crate::extract::Ctx;
use crate::web::Tab;
use askama::Template;
use axum::response::Html;
use axum::{Router, routing};

pub fn routes() -> Router<AppState> {
    Router::new().route("/upload", routing::get(upload))
}

#[derive(Template)]
#[template(path = "pages/upload.html")]
struct UploadTemplate {
    ctx: Context,
    active_tab: Tab,
}

async fn upload(ctx: Ctx) -> Html<String> {
    let Ctx(ctx, _) = ctx;
    UploadTemplate {
        ctx,
        active_tab: Tab::Upload,
    }
    .render()
    .map(Html)
    .unwrap()
}
