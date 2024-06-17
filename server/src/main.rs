pub mod api;
pub mod config;
pub mod func;
pub mod image;
pub mod math;
pub mod model;
pub mod query;
pub mod schema;
#[cfg(test)]
mod test;
pub mod util;

use diesel::prelude::*;
use warp::http::StatusCode;
use warp::Filter;

pub fn establish_connection() -> ConnectionResult<PgConnection> {
    let database_url = match std::env::var("DOCKER_DEPLOYMENT") {
        Ok(_) => "postgres://postgres:admin@host.docker.internal/booru",
        Err(_) => "postgres://postgres:admin@localhost/booru",
    };
    PgConnection::establish(&database_url)
}

#[tokio::main]
async fn main() {
    let get_info = warp::get()
        .and(warp::path("info"))
        .and(warp::path::end())
        .and_then(api::info::get_info);

    let catch_all = warp::any().map(|| {
        println!("Unimplemented request!");
        warp::reply::with_status("Bad Request", StatusCode::BAD_REQUEST)
    });

    let routes = get_info.or(catch_all);

    // Define the server address and run the warp server
    warp::serve(routes).run(([0, 0, 0, 0], 6666)).await;
}
