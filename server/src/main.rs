#![warn(clippy::pedantic)]
// Gives warnings on derives
#![allow(clippy::needless_for_each, clippy::large_stack_arrays, clippy::too_many_arguments)]
// Gives warnings for integer casts in const context
#![allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
// Option<Option<T>> is convenient for deserializing optional nullable JSON fields
#![allow(clippy::option_option)]
// Buggy
#![allow(clippy::iter_not_returning_iterator)]
// Unhelpful
#![allow(clippy::float_cmp)]
// Too subjective
#![allow(clippy::unreadable_literal, clippy::too_many_lines)]

mod admin;
mod api;
mod app;
mod auth;
mod autotag;
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
    let config = config::create();
    app::enable_tracing(&config);

    let auto_tagger = autotag::AutoTagSession::new(&config).unwrap_or_else(|err| {
        tracing::error!("Unable to intialize auto-tagger. Details:\n{err}");
        std::process::exit(1)
    });
    let state = app::AppState::new(db::create_connection_pool(), config, auto_tagger);

    app::initialize(&state).unwrap_or_else(|err| {
        tracing::error!("An error occurred during initialization. Details:\n{err}");
        std::process::exit(1)
    });
    app::run(state).await.unwrap_or_else(|err| {
        tracing::error!("Unable to start server. Details:\n{err}");
        std::process::exit(1)
    });
}
