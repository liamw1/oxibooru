use crate::api::middleware;
use crate::app::AppState;
use axum::http::Uri;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use axum::{Router, routing};

mod home;
mod nav;

pub fn routes(state: AppState) -> Router {
    home::routes()
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), middleware::auth))
        .route("/static/{*file}", routing::get(static_handler))
        .with_state(state)
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
