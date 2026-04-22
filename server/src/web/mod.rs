use crate::app::AppState;
use askama::Template;
use axum::http::Uri;
use axum::http::header::CONTENT_TYPE;
use axum::response::{Html, IntoResponse, Response};
use axum::{Router, routing};

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/", routing::get(test_handler))
        .route("/static/{*file}", routing::get(static_handler))
        .with_state(state)
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate;

async fn test_handler() -> Html<String> {
    Html(IndexTemplate.render().unwrap())
}

async fn static_handler(uri: Uri) -> Response {
    let mime_type = match uri.path().rsplit_once('.').map(|(_, ext)| ext) {
        Some("js") => "application/javascript",
        _ => panic!("Unknown MIME type!"),
    };

    let path = format!("{PROJECT_ROOT}/static/{}", uri.path().trim_start_matches("/static/"));
    let data = std::fs::read_to_string(path).unwrap();
    ([(CONTENT_TYPE, mime_type)], data).into_response()
}

const PROJECT_ROOT: &str = env!("CARGO_MANIFEST_DIR");
