use crate::admin::database;
use crate::config;
#[cfg(test)]
use crate::test;
use diesel::migration::Migration;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool, PoolError, PooledConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use std::borrow::Cow;
use std::sync::LazyLock;

pub type Connection = PooledConnection<ConnectionManager<PgConnection>>;
pub type ConnectionPool = Pool<ConnectionManager<PgConnection>>;
pub type ConnectionResult = Result<Connection, PoolError>;

/// Returns a connection to the database from a connection pool.
/// TODO: Try making this function async to increase throughput
pub fn get_connection() -> ConnectionResult {
    #[cfg(not(test))]
    {
        CONNECTION_POOL.get()
    }
    #[cfg(test)]
    {
        test::get_connection()
    }
}

#[cfg(test)]
pub fn get_prod_connection() -> ConnectionResult {
    CONNECTION_POOL.get()
}

/// Runs embedded migrations on the database. Used to update database for end-users who don't build server themselves.
/// Doesn't perform any error handling, as this is meant to be run once on application start.
pub fn run_migrations(conn: &mut PgConnection) {
    let pending_migrations = conn.pending_migrations(MIGRATIONS).unwrap();
    if pending_migrations.is_empty() {
        return;
    }

    println!("Running pending migrations...");
    conn.run_pending_migrations(MIGRATIONS).unwrap();
    if cfg!(test) {
        return;
    }

    let migration_number =
        |migration: &dyn Migration<Pg>| -> i32 { migration.name().version().to_string().parse().unwrap() };
    let first_migration = migration_number(pending_migrations.first().unwrap());
    let last_migration = migration_number(pending_migrations.last().unwrap());
    let migration_range = first_migration..=last_migration;

    // Update filenames if migrating primary keys to BIGINT
    if migration_range.contains(&12) {
        database::reset_filenames().unwrap();
    }

    // Cache thumbnail sizes if migrating to statistics system
    if migration_range.contains(&13) {
        database::reset_thumbnail_sizes(conn).unwrap();
    }
}

/// Returns a url for the database using `POSTGRES_USER`, `POSTGRES_PASSWORD`, and `POSTGRES_DATABASE` environment variables.
/// If `database_override` is not `None`, then it's value will be used in place of `POSTGRES_DATABASE`.
pub fn create_url(database_override: Option<&str>) -> String {
    if std::env::var("DOCKER_DEPLOYMENT").is_err() {
        dotenvy::from_filename("../.env").unwrap();
    }

    let user = std::env::var("POSTGRES_USER").unwrap();
    let password = std::env::var("POSTGRES_PASSWORD").unwrap();
    let database = database_override
        .map(Cow::Borrowed)
        .unwrap_or(Cow::Owned(std::env::var("POSTGRES_DB").unwrap()));
    let hostname = match std::env::var("DOCKER_DEPLOYMENT") {
        Ok(_) => "host.docker.internal",
        Err(_) => "localhost",
    };

    format!("postgres://{user}:{password}@{hostname}/{database}")
}

const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

static CONNECTION_POOL: LazyLock<ConnectionPool> = LazyLock::new(|| {
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
