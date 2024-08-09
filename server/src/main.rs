mod api;
mod auth;
pub mod config;
mod error;
mod filesystem;
mod image;
pub mod math;
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
}); // TODO: Make this an env variable

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
    let (_addr, server) = warp::serve(api::routes()).bind_with_graceful_shutdown(([0, 0, 0, 0], port), async {
        match tokio::signal::ctrl_c().await {
            Ok(()) => println!("Stopping server..."),
            Err(err) => eprintln!("Unable to listen for shutdown signal: {err}"),
        };
    });

    server.await;
}
