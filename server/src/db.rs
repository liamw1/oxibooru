use crate::admin::{AdminTask, database};
use crate::app::AppState;
use crate::content::signature::SIGNATURE_VERSION;
use crate::schema::database_statistics;
use crate::{app, config};
use diesel::migration::Migration;
use diesel::pg::Pg;
use diesel::r2d2::{ConnectionManager, CustomizeConnection, Pool, PoolError, PooledConnection};
use diesel::{ExpressionMethods, PgConnection, QueryDsl, QueryResult, RunQueryDsl};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use std::error::Error;
use std::num::ParseIntError;
use std::ops::RangeInclusive;
use std::time::Duration;
use tracing::{error, info};

pub type Connection = PooledConnection<ConnectionManager<PgConnection>>;
pub type ConnectionPool = Pool<ConnectionManager<PgConnection>>;
pub type ConnectionResult = Result<Connection, PoolError>;

/// Creates a connection pool to the database.
pub fn create_connection_pool() -> ConnectionPool {
    assert!(!cfg!(test), "Connection to production database disallowed in test build!");

    let num_tokio_threads = tokio::runtime::Handle::try_current()
        .map(|handle| handle.metrics().num_workers())
        .unwrap_or(1);
    let num_threads = std::cmp::max(num_tokio_threads, app::num_rayon_threads()) as u32;

    Pool::builder()
        .max_size(num_threads + 1)
        .max_lifetime(None)
        .idle_timeout(None)
        .test_on_check_out(true)
        .connection_customizer(Box::new(ConnectionInitialzier {}))
        .build(ConnectionManager::new(config::database_url()))
        .expect("Connection pool must be constructible")
}

/// Runs embedded migrations on the database. Used to update database for end-users who don't build server themselves.
pub fn run_database_migrations(
    connection_pool: &ConnectionPool,
) -> Result<RangeInclusive<i32>, Box<dyn Error + Send + Sync>> {
    let mut conn = connection_pool.get()?;
    let pending_migrations = conn.pending_migrations(MIGRATIONS)?;
    if pending_migrations.is_empty() {
        return Ok(RangeInclusive::new(1, 0));
    }

    let migration_number = |migration: &dyn Migration<Pg>| -> Result<i32, ParseIntError> {
        migration.name().version().to_string().parse()
    };

    let panic_message = "There must be at least one migration";
    let first_migration = migration_number(pending_migrations.first().expect(panic_message))?;
    let last_migration = migration_number(pending_migrations.last().expect(panic_message))?;
    let migration_range = first_migration..=last_migration;

    info!("Running pending migrations...");
    conn.run_pending_migrations(MIGRATIONS)?;
    Ok(migration_range)
}

pub fn run_server_migrations(
    state: &AppState,
    migration_range: RangeInclusive<i32>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Update filenames if migrating primary keys to BIGINT
    if migration_range.contains(&12) && !migration_range.contains(&1) {
        database::reset_filenames(state)?;
    }

    // If creating the database for the first time, set post signature version
    let mut conn = state.get_connection()?;
    if migration_range.contains(&1) {
        diesel::update(database_statistics::table)
            .set(database_statistics::signature_version.eq(SIGNATURE_VERSION))
            .execute(&mut conn)?;
    }

    // Cache thumbnail sizes if migrating to statistics system
    if migration_range.contains(&13) {
        database::reset_thumbnail_sizes(state)?;
    }
    Ok(())
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

#[derive(Debug)]
struct ConnectionInitialzier {}

impl CustomizeConnection<PgConnection, diesel::r2d2::Error> for ConnectionInitialzier {
    fn on_acquire(&self, conn: &mut PgConnection) -> Result<(), diesel::r2d2::Error> {
        let config = config::create();
        if config.auto_explain {
            diesel::sql_query("LOAD 'auto_explain';").execute(conn)?;
            diesel::sql_query("SET SESSION auto_explain.log_min_duration = 500;").execute(conn)?;
            diesel::sql_query("SET SESSION auto_explain.log_parameter_max_length = 0;").execute(conn)?;
        }
        Ok(())
    }

    fn on_release(&self, _conn: PgConnection) {}
}
