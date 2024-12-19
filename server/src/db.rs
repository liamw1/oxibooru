use crate::config;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool, PoolError, PooledConnection};
use diesel::result::Error;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use std::sync::LazyLock;

/// Returns a connection to the database from a connection pool.
pub fn get_connection() -> Result<PooledConnection<ConnectionManager<PgConnection>>, PoolError> {
    CONNECTION_POOL.get()
}

/// Runs embedded migrations on the database. Used to update database for end-users who don't build server themselves.
/// Doesn't perform any error handling, as this is meant to be run once on application start.
pub fn run_migrations() {
    get_connection().unwrap().run_pending_migrations(MIGRATIONS).unwrap();
}

/// Executes `function` in a transaction and retries up to `max_retries` times if it fails due to a deadlock.
pub fn deadlock_prone_transaction<T, E, F>(conn: &mut PgConnection, max_retries: u32, function: F) -> Result<T, E>
where
    F: Fn(&mut PgConnection) -> Result<T, E>,
    E: From<Error> + std::error::Error,
{
    let print_info = |num_retries: u32, result: Result<T, E>| {
        if num_retries > 0 {
            eprintln!("{num_retries} deadlocks detected!");
        }
        result
    };

    let mut result = conn.transaction(&function);
    for retry in 0..max_retries {
        result = match result {
            Ok(_) => return print_info(retry, result),
            Err(err) if err.to_string().contains("deadlock") => conn.transaction(&function),
            Err(_) => return print_info(retry, result),
        };
    }
    print_info(max_retries, result)
}

const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

static CONNECTION_POOL: LazyLock<Pool<ConnectionManager<PgConnection>>> = LazyLock::new(|| {
    let num_threads = tokio::runtime::Handle::try_current()
        .map(|handle| handle.metrics().num_workers())
        .unwrap_or(1);
    let manager = ConnectionManager::new(config::database_url());
    Pool::builder()
        .max_size(num_threads as u32)
        .max_lifetime(None)
        .idle_timeout(None)
        .test_on_check_out(true)
        .build(manager)
        .expect("Could not build connection pool")
});
