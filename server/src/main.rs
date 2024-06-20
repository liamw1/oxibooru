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

const DEFAULT_PORT: u16 = 6666;
static DATABASE_URL: Lazy<&'static str> = Lazy::new(|| match std::env::var("DOCKER_DEPLOYMENT") {
    Ok(_) => "postgres://postgres:admin@host.docker.internal/booru",
    Err(_) => "postgres://postgres:admin@localhost/booru",
});

fn establish_connection() -> ConnectionResult<PgConnection> {
    PgConnection::establish(&DATABASE_URL)
}

#[tokio::main]
async fn main() {
    // Define the server address and run the warp server
    let port: u16 = std::env::var("PORT")
        .map(|var| var.parse().unwrap_or(DEFAULT_PORT))
        .unwrap_or(DEFAULT_PORT);
    warp::serve(api::routes()).run(([0, 0, 0, 0], port)).await;
}
