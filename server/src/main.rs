#![warn(clippy::pedantic)]
// Gives warnings on EnumTables
#![allow(clippy::too_many_arguments)]
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

#[tokio::main]
async fn main() {
    let state = app::AppState::new(db::create_connection_pool(), config::create());

    app::enable_tracing(&state);
    if let Err(err) = app::initialize(&state) {
        tracing::error!("An error occurred during initialization. Details:\n{err}");
        std::process::exit(1);
    }
    if let Err(err) = app::run(state).await {
        tracing::error!("Unable to start server. Details:\n{err}");
        std::process::exit(1);
    }
}
