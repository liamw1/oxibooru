#![allow(clippy::too_many_arguments)]

use tracing::error;

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

    // Connection is placed in scope so that it is dropped before starting server
    {
        let mut conn = match db::get_connection() {
            Ok(conn) => conn,
            Err(err) => {
                error!("Could not establish a connection to the database. Details:\n\n{err}");
                return;
            }
        };
        if let Err(err) = db::run_migrations(&mut conn) {
            error!("Failed to run migrations. Details:\n\n{err}");
            return;
        }

        if admin::enabled() {
            return admin::command_line_mode(&mut conn);
        }
        if let Err(err) = db::check_signature_version(&mut conn) {
            error!("An error occured while checking signature version: {err}");
            return;
        }
    }

    filesystem::purge_temporary_uploads().unwrap();
    app::run().await;
}
