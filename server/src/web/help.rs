use crate::app::AppState;
use crate::config::Action;
use crate::extract::Ctx;
use crate::web::WebError;
use crate::web::WebResult;
use crate::web::{Html, Tab};
use askama::Template;
use axum::{Router, routing};

pub fn routes() -> Router<AppState> {
    let search_routes = Router::new()
        .route("/", routing::get(search_general))
        .route("/posts", routing::get(search_posts))
        .route("/users", routing::get(search_users))
        .route("/tags", routing::get(search_tags))
        .route("/pools", routing::get(search_pools));
    let help_routes = Router::new()
        .route("/", routing::get(about))
        .route("/about", routing::get(about))
        .route("/keyboard", routing::get(keyboard))
        .route("/comments", routing::get(comments))
        .route("/tos", routing::get(tos))
        .nest("/search", search_routes);
    Router::new().nest("/help", help_routes)
}

#[derive(PartialEq, Eq)]
enum HelpTab {
    About,
    Keyboard,
    Search,
    Comments,
    Tos,
}

#[derive(PartialEq, Eq)]
enum SearchTab {
    General,
    Posts,
    Users,
    Tags,
    Pools,
}

#[derive(Template)]
#[template(path = "pages/help.html")]
struct Help {
    ctx: Ctx,
    active_tab: Tab,
    active_help_tab: HelpTab,
    active_search_tab: SearchTab,
}

fn regular_page(ctx: Ctx, active_help_tab: HelpTab) -> WebResult<Html> {
    Help {
        ctx,
        active_tab: Tab::Help,
        active_help_tab,
        active_search_tab: SearchTab::General,
    }
    .render()
    .map(Html)
    .map_err(WebError::from)
}

fn search_page(ctx: Ctx, active_search_tab: SearchTab) -> WebResult<Html> {
    Help {
        ctx,
        active_tab: Tab::Help,
        active_help_tab: HelpTab::Search,
        active_search_tab,
    }
    .render()
    .map(Html)
    .map_err(WebError::from)
}

async fn about(ctx: Ctx) -> WebResult<Html> {
    regular_page(ctx, HelpTab::About)
}

async fn keyboard(ctx: Ctx) -> WebResult<Html> {
    regular_page(ctx, HelpTab::Keyboard)
}

async fn comments(ctx: Ctx) -> WebResult<Html> {
    regular_page(ctx, HelpTab::Comments)
}

async fn tos(ctx: Ctx) -> WebResult<Html> {
    regular_page(ctx, HelpTab::Tos)
}

async fn search_general(ctx: Ctx) -> WebResult<Html> {
    search_page(ctx, SearchTab::General)
}

async fn search_posts(ctx: Ctx) -> WebResult<Html> {
    search_page(ctx, SearchTab::Posts)
}

async fn search_users(ctx: Ctx) -> WebResult<Html> {
    search_page(ctx, SearchTab::Users)
}

async fn search_tags(ctx: Ctx) -> WebResult<Html> {
    search_page(ctx, SearchTab::Tags)
}

async fn search_pools(ctx: Ctx) -> WebResult<Html> {
    search_page(ctx, SearchTab::Pools)
}
