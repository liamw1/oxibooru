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

#[derive(Default)]
struct Settings {
    posts_per_page: u64,
    keyboard_shortcuts: bool,
    dark_theme: bool,
    upscale_small_posts: bool,
    endless_scroll: bool,
    post_flow: bool,
    transparency_grid: bool,
    tag_suggestions: bool,
    autoplay_videos: bool,
    underscores_as_spaces: bool,
}

#[derive(Template)]
#[template(path = "pages/settings.html")]
struct SettingsTemplate {
    ctx: Context,
    active_tab: Tab,
    settings: Settings,
}

async fn settings(ctx: Ctx) -> Html<String> {
    let settings = Settings {
        posts_per_page: 42,
        keyboard_shortcuts: true,
        transparency_grid: true,
        tag_suggestions: true,
        ..Default::default()
    };

    let Ctx(ctx, _) = ctx;
    SettingsTemplate {
        ctx,
        active_tab: Tab::Settings,
        settings,
    }
    .render()
    .map(Html)
    .unwrap()
}
