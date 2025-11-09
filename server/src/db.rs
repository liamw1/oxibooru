use crate::admin::{AdminTask, database};
use crate::content::signature::SIGNATURE_VERSION;
use crate::schema::database_statistics;
#[cfg(test)]
use crate::test;
use crate::{app, config};
use diesel::migration::Migration;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, CustomizeConnection, Pool, PoolError, PooledConnection};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use std::error::Error;
use std::num::ParseIntError;
use std::sync::LazyLock;
use std::time::Duration;
use tracing::{error, info};

pub type Connection = PooledConnection<ConnectionManager<PgConnection>>;
pub type ConnectionPool = Pool<ConnectionManager<PgConnection>>;
pub type ConnectionResult = Result<Connection, PoolError>;

/// Returns a connection to the database from a connection pool.
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
pub fn run_migrations(conn: &mut PgConnection) -> Result<(), Box<dyn Error + Send + Sync>> {
    let pending_migrations = conn.pending_migrations(MIGRATIONS)?;
    if pending_migrations.is_empty() {
        return Ok(());
    }

    let migration_number = |migration: &dyn Migration<Pg>| -> Result<i32, ParseIntError> {
        migration.name().version().to_string().parse()
    };

    let panic_message = "There must be at least one migration";
    let first_migration = migration_number(pending_migrations.first().expect(panic_message))?;
    let last_migration = migration_number(pending_migrations.last().expect(panic_message))?;
    let migration_range = first_migration..=last_migration;

    // Update filenames if migrating primary keys to BIGINT
    if migration_range.contains(&12) && !migration_range.contains(&1) {
        database::reset_filenames()?;
    }

    info!("Running pending migrations...");
    conn.run_pending_migrations(MIGRATIONS)?;
    if cfg!(test) {
        return Ok(());
    }

    // If creating the database for the first time, set post signature version
    if migration_range.contains(&1) {
        diesel::update(database_statistics::table)
            .set(database_statistics::signature_version.eq(SIGNATURE_VERSION))
            .execute(conn)?;
    }

    // Cache thumbnail sizes if migrating to statistics system
    if migration_range.contains(&13) {
        database::reset_thumbnail_sizes(conn)?;
    }
    Ok(())
}

/// Returns a url for the database using `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_HOST`, and `POSTGRES_DATABASE`
/// environment variables. If `database_override` is not `None`, then it's value will be used in place of `POSTGRES_DATABASE`.
pub fn create_url(database_override: Option<&str>) -> String {
    if std::env::var("DOCKER_DEPLOYMENT").is_err() {
        dotenvy::from_filename("../.env").expect(".env must be in project root directory");
    }

    let user = std::env::var("POSTGRES_USER").expect("POSTGRES_USER must be defined in .env");
    let password = std::env::var("POSTGRES_PASSWORD").expect("POSTGRES_PASSWORD must be defined in .env");
    let hostname = std::env::var("POSTGRES_HOST").unwrap_or_else(|_| String::from("localhost"));
    let database = std::env::var("POSTGRES_DB").expect("POSTGRES_DB must be defined in .env");
    let database = database_override.unwrap_or(&database);

    format!("postgres://{user}:{password}@{hostname}/{database}")
}

pub fn check_signature_version(conn: &mut PgConnection) -> QueryResult<()> {
    let mut get_current_version = || -> QueryResult<i32> {
        database_statistics::table
            .select(database_statistics::signature_version)
            .first(conn)
    };

    if get_current_version()? == SIGNATURE_VERSION {
        return Ok(());
    }

    let task: &str = AdminTask::RecomputePostSignatures.into();
    error!(
        "Post signatures are out of date and need to be recomputed.

        This can be done via the admin cli, which can be entered by passing
        the --admin flag to the server executable. If you are deploying with
        docker, you can do this by navigating to the source directory and
        executing the following command:
        
           docker exec -it oxibooru-server-1 ./server --admin
            
        While in the admin cli, simply run the {task} task.
        Once this task has started, this server instance will resume operations
        while the signatures recompute in the background. Reverse search may be
        inaccurate during this process, so you may wish to suspend post uploads
        until the task completes."
    );
    while get_current_version()? != SIGNATURE_VERSION {
        std::thread::sleep(Duration::from_secs(1));
    }
    Ok(())
}

const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

static CONNECTION_POOL: LazyLock<ConnectionPool> = LazyLock::new(|| {
    let num_tokio_threads = tokio::runtime::Handle::try_current()
        .map(|handle| handle.metrics().num_workers())
        .unwrap_or(1);
    let num_threads = std::cmp::max(num_tokio_threads, app::num_rayon_threads()) as u32;

    let manager = ConnectionManager::new(config::database_url());
    Pool::builder()
        .max_size(num_threads + 1)
        .max_lifetime(None)
        .idle_timeout(None)
        .test_on_check_out(true)
        .connection_customizer(Box::new(ConnectionInitialzier {}))
        .build(manager)
        .expect("Connection pool must be constructible")
});

#[derive(Debug)]
struct ConnectionInitialzier {}

impl CustomizeConnection<PgConnection, diesel::r2d2::Error> for ConnectionInitialzier {
    fn on_acquire(&self, conn: &mut PgConnection) -> Result<(), diesel::r2d2::Error> {
        if config::get().auto_explain {
            diesel::sql_query("LOAD 'auto_explain';").execute(conn)?;
            diesel::sql_query("SET SESSION auto_explain.log_min_duration = 500;").execute(conn)?;
            diesel::sql_query("SET SESSION auto_explain.log_parameter_max_length = 0;").execute(conn)?;
        }
        Ok(())
    }

    fn on_release(&self, _conn: PgConnection) {}
}
