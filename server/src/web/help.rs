use crate::app::AppState;
use crate::app::Context;
use crate::config::Action;
use crate::extract::Ctx;
use crate::web::Tab;
use askama::Template;
use axum::response::Html;
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
    ctx: Context,
    active_tab: Tab,
    active_help_tab: HelpTab,
    active_search_tab: SearchTab,
}

impl Help {
    fn regular(ctx: Context, active_help_tab: HelpTab) -> Self {
        Self {
            ctx,
            active_tab: Tab::Help,
            active_help_tab,
            active_search_tab: SearchTab::General,
        }
    }

    fn search(ctx: Context, active_search_tab: SearchTab) -> Self {
        Self {
            ctx,
            active_tab: Tab::Help,
            active_help_tab: HelpTab::Search,
            active_search_tab,
        }
    }
}

async fn about(Ctx(ctx, _): Ctx) -> Html<String> {
    Help::regular(ctx, HelpTab::About).render().map(Html).unwrap()
}

async fn keyboard(Ctx(ctx, _): Ctx) -> Html<String> {
    Help::regular(ctx, HelpTab::Keyboard).render().map(Html).unwrap()
}

async fn comments(Ctx(ctx, _): Ctx) -> Html<String> {
    Help::regular(ctx, HelpTab::Comments).render().map(Html).unwrap()
}

async fn tos(Ctx(ctx, _): Ctx) -> Html<String> {
    Help::regular(ctx, HelpTab::Tos).render().map(Html).unwrap()
}

async fn search_general(Ctx(ctx, _): Ctx) -> Html<String> {
    Help::search(ctx, SearchTab::General).render().map(Html).unwrap()
}

async fn search_posts(Ctx(ctx, _): Ctx) -> Html<String> {
    Help::search(ctx, SearchTab::Posts).render().map(Html).unwrap()
}

async fn search_users(Ctx(ctx, _): Ctx) -> Html<String> {
    Help::search(ctx, SearchTab::Users).render().map(Html).unwrap()
}

async fn search_tags(Ctx(ctx, _): Ctx) -> Html<String> {
    Help::search(ctx, SearchTab::Tags).render().map(Html).unwrap()
}

async fn search_pools(Ctx(ctx, _): Ctx) -> Html<String> {
    Help::search(ctx, SearchTab::Pools).render().map(Html).unwrap()
}
