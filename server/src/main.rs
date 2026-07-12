#![warn(clippy::pedantic)]
// Gives warnings on derives
#![allow(clippy::needless_for_each, clippy::large_stack_arrays, clippy::too_many_arguments)]
// Gives warnings for integer casts in const context
#![allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
// Option<Option<T>> is convenient for deserializing optional nullable JSON fields
#![allow(clippy::option_option)]
// Buggy: Fires when using assert_eq! to compare to 0.0
#![allow(clippy::float_cmp)]
// Too subjective
#![allow(clippy::unreadable_literal, clippy::too_many_lines)]

mod admin;
mod api;
mod app;
mod auth;
mod config;
mod content;
mod db;
mod error;
mod extract;
mod filesystem;
mod math;
mod model;
mod resource;
mod schema;
mod search;
mod snapshot;
mod string;
#[cfg(test)]
mod test;
mod time;
mod update;

// Avoid musl's default allocator due to lackluster performance
// https://nickb.dev/blog/default-musl-allocator-considered-harmful-to-performance
#[cfg(target_env = "musl")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() {
    #[cfg(feature = "load_env")]
    app::load_env().unwrap_or_else(|err| app::shutdown("Failed to load .env", err));

    // Enable logging
    let config = config::create();
    app::enable_tracing(&config);

    // Create global app state
    let downloader = content::download::create_client()
        .unwrap_or_else(|err| app::shutdown("Unable to create downloader client", err));
    let connection_pool = db::create_connection_pool(config.clone())
        .unwrap_or_else(|err| app::shutdown("Unable to build connection pool", err));
    let state = app::AppState::new(downloader, connection_pool, config);

    // Initialize and run server
    app::initialize(&state).unwrap_or_else(|err| app::shutdown("An error occured during initialization", err));
    app::run(state)
        .await
        .unwrap_or_else(|err| app::shutdown("Unable to start server", err));
}
