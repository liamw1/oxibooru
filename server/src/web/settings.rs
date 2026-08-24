use crate::app::{AppState, Context};
use crate::config::Action;
use crate::extract::Ctx;
use crate::web::Tab;
use askama::Template;
use axum::response::Html;
use axum::{Router, routing};

pub fn routes() -> Router<AppState> {
    Router::new().route("/settings", routing::get(settings))
}

#[derive(Template)]
#[template(path = "pages/settings.html")]
struct SettingsTemplate {
    ctx: Context,
    active_tab: Tab,
}

async fn settings(ctx: Ctx) -> Html<String> {
    let Ctx(ctx, _) = ctx;
    SettingsTemplate {
        ctx,
        active_tab: Tab::Settings,
    }
    .render()
    .map(Html)
    .unwrap()
}
