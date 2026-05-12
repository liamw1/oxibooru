use crate::app::{AppState, Context};
use crate::config::Action;
use crate::extract::Ctx;
use crate::web::nav::Nav;
use askama::Template;
use axum::response::Html;
use axum::{Router, routing};

pub fn routes() -> Router<AppState> {
    Router::new().route("/", routing::get(home))
}

#[derive(Template)]
#[template(path = "pages/home.html")]
struct HomeTemplate {
    nav: Nav,
    ctx: Context,
}

async fn home(Ctx(ctx, _): Ctx) -> Html<String> {
    let nav = Nav::create(&ctx);
    Html(HomeTemplate { nav, ctx }.render().unwrap())
}
