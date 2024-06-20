pub mod api;
pub mod auth;
pub mod config;
pub mod image;
pub mod math;
pub mod model;
pub mod query;
pub mod schema;
#[cfg(test)]
mod test;
pub mod util;

use diesel::prelude::*;
use model::rank::UserRank;
use once_cell::sync::Lazy;
use warp::http::StatusCode;
use warp::Filter;

pub fn establish_connection() -> ConnectionResult<PgConnection> {
    PgConnection::establish(&DATABASE_URL)
}

const DEFAULT_PORT: u16 = 6666;
static DATABASE_URL: Lazy<&'static str> = Lazy::new(|| match std::env::var("DOCKER_DEPLOYMENT") {
    Ok(_) => "postgres://postgres:admin@host.docker.internal/booru",
    Err(_) => "postgres://postgres:admin@localhost/booru",
});

#[tokio::main]
async fn main() {
    let auth = warp::header::optional("Authorization").map(|_token: Option<String>| UserRank::Anonymous);
    let log = warp::filters::log::custom(|info| println!("{} {} [{}]", info.method(), info.path(), info.status()));

    let get_info = warp::get().and(warp::path!("info")).and_then(api::info::get_info);
    let list_tag_categories = warp::get()
        .and(warp::path!("tag-categories"))
        .and(auth)
        .and_then(api::tag_category::list_tag_categories);
    let list_pool_categories = warp::get()
        .and(warp::path!("pool-categories"))
        .and(auth)
        .and_then(api::pool_category::list_pool_categories);
    let post_user = warp::post()
        .and(warp::path!("users"))
        .and(warp::body::bytes())
        .and_then(api::user::post_user);

    let catch_all = warp::any().map(|| {
        println!("Unimplemented request!");
        warp::reply::with_status("Bad Request", StatusCode::BAD_REQUEST)
    });

    let routes = get_info
        .or(list_tag_categories)
        .or(list_pool_categories)
        .or(post_user)
        .or(catch_all)
        .with(log);

    // Define the server address and run the warp server
    let port: u16 = std::env::var("PORT")
        .map(|var| var.parse().unwrap_or(DEFAULT_PORT))
        .unwrap_or(DEFAULT_PORT);
    warp::serve(routes).run(([0, 0, 0, 0], port)).await;
}
