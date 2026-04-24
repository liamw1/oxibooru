use crate::api::{self, middleware};
use crate::app::AppState;
use crate::auth::Client;
use crate::config::Config;
use askama::Template;
use axum::extract::{Extension, State};
use axum::http::Uri;
use axum::http::header::CONTENT_TYPE;
use axum::response::{Html, IntoResponse, Response};
use axum::{Router, routing};

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/", routing::get(home))
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), middleware::auth))
        .route("/static/{*file}", routing::get(static_handler))
        .with_state(state)
}

struct NavItem {
    title: &'static str,
    key: String,
    url: String,
}

impl NavItem {
    fn new(title: &'static str) -> Self {
        let key = title.to_lowercase().split_whitespace().collect();
        let url = format!("/{key}");
        Self { title, key, url }
    }
}

struct Nav {
    items: Vec<NavItem>,
}

impl Nav {
    fn create(config: &Config, client: Client) -> Self {
        let mut items = Vec::new();

        items.push(NavItem::new("Home"));

        let privileges = &config.privileges();
        api::verify_privilege(client, privileges.post_list)
            .is_ok()
            .then(|| items.push(NavItem::new("Posts")));
        api::verify_privilege(client, privileges.upload_create)
            .is_ok()
            .then(|| items.push(NavItem::new("Upload")));
        api::verify_privilege(client, privileges.comment_list)
            .is_ok()
            .then(|| items.push(NavItem::new("Comments")));
        api::verify_privilege(client, privileges.tag_list)
            .is_ok()
            .then(|| items.push(NavItem::new("Tags")));
        api::verify_privilege(client, privileges.pool_list)
            .is_ok()
            .then(|| items.push(NavItem::new("Pools")));
        api::verify_privilege(client, privileges.user_list)
            .is_ok()
            .then(|| items.push(NavItem::new("Users")));

        if client.id.is_some() {
            items.push(NavItem {
                title: "Account",
                key: "account".into(),
                url: format!("/user/{}", "test_user"),
            });
            items.push(NavItem::new("Logout"));
        } else {
            items.push(NavItem::new("Register"));
            items.push(NavItem::new("Log in"));
        }

        items.push(NavItem::new("Help"));

        Self { items }
    }
}

#[derive(Template)]
#[template(path = "pages/home.html")]
struct HomeTemplate<'a> {
    nav: Nav,
    config: &'a Config,
    client: Client,
}

async fn home(State(state): State<AppState>, Extension(client): Extension<Client>) -> Html<String> {
    let nav = Nav::create(&state.config, client);
    let config = &state.config;
    Html(HomeTemplate { nav, config, client }.render().unwrap())
}

/// Temporary: Replace with a better function eventually
async fn static_handler(uri: Uri) -> Response {
    let mime_type = match uri.path().rsplit_once('.').map(|(_, ext)| ext) {
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("png") => "image/png",
        Some("woff2") => "font/ttf",
        Some(ext) => panic!("Unknown MIME type {ext}!"),
        None => panic!("No extension!"),
    };

    let path = format!("{PROJECT_ROOT}/static/{}", uri.path().trim_start_matches("/static/"));
    let data = std::fs::read(path).unwrap();
    ([(CONTENT_TYPE, mime_type)], data).into_response()
}

const PROJECT_ROOT: &str = env!("CARGO_MANIFEST_DIR");
