#![allow(clippy::too_many_arguments)]

mod admin;
mod api;
mod auth;
mod config;
mod content;
mod db;
mod error;
mod filesystem;
mod math;
mod model;
mod resource;
mod schema;
mod search;
#[cfg(test)]
mod test;
mod time;
mod update;

#[tokio::main]
async fn main() {
    if admin::run_tasks() > 0 {
        return;
    }

    println!("Oxibooru server running on {} threads", tokio::runtime::Handle::current().metrics().num_workers());
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
