#![warn(clippy::pedantic)]
// Gives warnings on EnumTables
#![allow(clippy::too_many_arguments)]
// Gives warnings for every diesel::prelude::* import
#![allow(clippy::wildcard_imports)]
// Option<Option<T>> is convenient for deserializing optional nullable JSON fields
#![allow(clippy::option_option)]
// Buggy
#![allow(clippy::iter_not_returning_iterator)]
// Too subjective
#![allow(
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::single_match_else,
    clippy::match_bool
)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::match_same_arms)]

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
    app::enable_tracing();
    if let Err(err) = app::initialize() {
        tracing::error!("An error occurred during initialization. Details:\n{err}");
        std::process::exit(1);
    }
    app::run().await;
}
