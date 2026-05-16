use crate::api::middleware;
use crate::app::AppState;
use axum::Router;
use serde::Deserialize;
use tower_http::services::ServeDir;

mod home;
mod post;

pub fn routes(state: AppState) -> Router {
    // TODO: Remove
    dotenvy::from_filename("../.env").unwrap();
    let data_dir = std::env::var("MOUNT_DATA").unwrap();
    let static_dir = format!("{PROJECT_ROOT}/static");

    home::routes()
        .merge(post::routes())
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), middleware::auth))
        .nest_service("/data", ServeDir::new(&data_dir))
        .nest_service("/static", ServeDir::new(&static_dir))
        .with_state(state)
}

#[derive(Deserialize)]
struct SearchQuery {
    query: Option<String>,
}

const PROJECT_ROOT: &str = env!("CARGO_MANIFEST_DIR");
