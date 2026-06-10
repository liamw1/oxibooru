use crate::api::middleware;
use crate::app::AppState;
use axum::Router;
use serde::Serialize;
use tower_http::services::ServeDir;

mod help;
mod home;
mod pager;
mod pool;
mod pool_category;
mod post;
mod tag;
mod tag_category;

pub fn post_url<T: Serialize>(post_id: i64, params: &T) -> Result<String, serde_urlencoded::ser::Error> {
    let base = format!("/post/{post_id}");
    url(&base, params)
}

pub fn routes(state: AppState) -> Router {
    // TODO: Remove
    dotenvy::from_filename("../.env").unwrap();
    let data_dir = std::env::var("MOUNT_DATA").unwrap();
    let static_dir = format!("{PROJECT_ROOT}/static");

    help::routes()
        .merge(home::routes())
        .merge(pool::routes())
        .merge(post::routes())
        .merge(tag::routes())
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), middleware::auth))
        .nest_service("/data", ServeDir::new(&data_dir))
        .nest_service("/static", ServeDir::new(&static_dir))
        .with_state(state)
}

#[derive(PartialEq, Eq)]
enum Tab {
    Home,
    Post,
    Upload,
    Comment,
    Tag,
    Pool,
    User,
    Account,
    Login,
    Help,
    Settings,
}

const PROJECT_ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn url<T: Serialize>(base: &str, params: &T) -> Result<String, serde_urlencoded::ser::Error> {
    serde_urlencoded::to_string(params).map(|query_string| {
        if query_string.is_empty() {
            base.to_owned()
        } else {
            format!("{base}?{query_string}")
        }
    })
}
