#![allow(clippy::too_many_arguments)]

mod admin;
mod api;
mod app;
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
mod string;
#[cfg(test)]
mod test;
mod time;
mod update;

#[tokio::main]
async fn main() {
    app::enable_tracing();
    if let Err(err) = app::initialize() {
        tracing::error!("An error occurred during initialization. Details:\n\n{err}");
        std::process::exit(1);
    }
    app::run().await;
}
