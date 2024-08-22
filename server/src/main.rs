#![allow(clippy::too_many_arguments)]

mod admin;
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
use diesel::r2d2::{ConnectionManager, Pool, PoolError, PooledConnection};
use std::sync::LazyLock;

static CONNECTION_POOL: LazyLock<Pool<ConnectionManager<PgConnection>>> = LazyLock::new(|| {
    let manager = ConnectionManager::new(config::database_url());
    Pool::builder()
        .test_on_check_out(true)
        .build(manager)
        .expect("Could not build connection pool")
});

fn get_connection() -> Result<PooledConnection<ConnectionManager<PgConnection>>, PoolError> {
    CONNECTION_POOL.get()
}

#[tokio::main]
async fn main() {
    if admin::run_tasks() > 0 {
        return;
    }
    filesystem::purge_temporary_uploads().unwrap();

    // Run the warp server
    let (_addr, server) =
        warp::serve(api::routes()).bind_with_graceful_shutdown(([0, 0, 0, 0], config::port()), async {
            match tokio::signal::ctrl_c().await {
                Ok(()) => println!("Stopping server..."),
                Err(err) => eprintln!("Unable to listen for shutdown signal: {err}"),
            };
        });

    server.await;
}
