mod api;
mod auth;
pub mod config;
mod error;
mod filesystem;
mod image;
mod math;
mod model;
mod resource;
mod schema;
mod search;
#[cfg(test)]
mod test;
mod update;
mod util;

use diesel::prelude::*;
use std::sync::LazyLock;

const DEFAULT_PORT: u16 = 6666;
static DATABASE_URL: LazyLock<&'static str> = LazyLock::new(|| match std::env::var("DOCKER_DEPLOYMENT") {
    Ok(_) => "postgres://postgres:postgres@host.docker.internal/booru",
    Err(_) => "postgres://postgres:postgres@localhost/booru",
});

fn establish_connection() -> ConnectionResult<PgConnection> {
    PgConnection::establish(&DATABASE_URL)
}

#[tokio::main]
async fn main() {
    filesystem::purge_temporary_uploads().unwrap();

    // Define the server address and run the warp server
    let port: u16 = std::env::var("PORT")
        .map(|var| var.parse().unwrap_or(DEFAULT_PORT))
        .unwrap_or(DEFAULT_PORT);
    warp::serve(api::routes()).run(([0, 0, 0, 0], port)).await;
}
