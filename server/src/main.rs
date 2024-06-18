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
    // let print_headers = warp::any()
    //     .and(warp::header::headers_cloned())
    //     .map(|h| println!("{:?}", h));

    let get_info = warp::get().and(warp::path!("info")).and_then(api::info::get_info);

    let catch_all = warp::any().map(|| {
        println!("Unimplemented request!");
        warp::reply::with_status("Bad Request", StatusCode::BAD_REQUEST)
    });

    let routes = get_info.or(catch_all);

    // Define the server address and run the warp server
    let port: u16 = std::env::var("PORT")
        .map(|var| var.parse().unwrap_or(DEFAULT_PORT))
        .unwrap_or(DEFAULT_PORT);
    warp::serve(routes).run(([0, 0, 0, 0], port)).await;
}
