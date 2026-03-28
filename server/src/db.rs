use crate::admin::AdminTask;
use crate::app::AppState;
use crate::content::signature::SIGNATURE_VERSION;
use crate::schema::database_statistics;
use crate::{admin, app, config};
use diesel::migration::Migration;
use diesel::pg::Pg;
use diesel::r2d2::{ConnectionManager, CustomizeConnection, Pool, PoolError, PooledConnection};
use diesel::{ExpressionMethods, PgConnection, QueryDsl, QueryResult, RunQueryDsl};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use std::error::Error;
use std::num::ParseIntError;
use std::ops::{Deref, DerefMut, RangeInclusive};
use std::time::Duration;
use tokio::sync::{Semaphore, SemaphorePermit};
use tracing::{error, info};

pub type Connection = PooledConnection<ConnectionManager<PgConnection>>;
pub type ConnectionResult = Result<Connection, PoolError>;
pub type AsyncConnectionResult<'a> = Result<AsyncConnection<'a>, PoolError>;

pub struct AsyncConnection<'a> {
    conn: Connection,
    _permit: SemaphorePermit<'a>,
}

impl Deref for AsyncConnection<'_> {
    type Target = Connection;
    fn deref(&self) -> &Self::Target {
        &self.conn
    }
}

impl DerefMut for AsyncConnection<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.conn
    }
}

pub struct AsyncConnectionPool {
    pool: Pool<ConnectionManager<PgConnection>>,
    semaphore: Semaphore,
}

impl AsyncConnectionPool {
    pub async fn get(&self) -> AsyncConnectionResult<'_> {
        let _permit = self.semaphore.acquire().await.expect("Semaphore should never close");
        let conn = self.pool.get()?;
        Ok(AsyncConnection { conn, _permit })
    }

    pub fn get_blocking(&self) -> Result<Connection, PoolError> {
        self.pool.get()
    }
}

/// Creates a connection pool to the database.
pub fn create_connection_pool() -> AsyncConnectionPool {
    if cfg!(test) {
        panic!("Connection to production database disallowed in test build!")
    } else {
        let num_tokio_threads = tokio::runtime::Handle::try_current()
            .map(|handle| handle.metrics().num_workers())
            .unwrap_or(1);
        let max_conns = std::cmp::max(num_tokio_threads, app::num_rayon_threads()) + 1;

        let pool = Pool::builder()
            .max_size(u32::try_from(max_conns).expect("Number of connections will never be greater than u32::MAX"))
            .max_lifetime(None)
            .idle_timeout(None)
            .test_on_check_out(true)
            .connection_customizer(Box::new(ConnectionInitialzier {}))
            .build(ConnectionManager::new(config::database_url(None)))
            .expect("Connection pool must be constructible");
        let semaphore = Semaphore::new(max_conns);
        AsyncConnectionPool { pool, semaphore }
    }
}

#[cfg(test)]
pub fn create_test_connection_pool(test_url: String) -> AsyncConnectionPool {
    let pool = Pool::builder()
        .max_lifetime(None)
        .idle_timeout(None)
        .test_on_check_out(true)
        .build(ConnectionManager::new(test_url))
        .expect("Test connection pool must be constructible");
    let semaphore = Semaphore::new(usize::try_from(pool.max_size()).unwrap());
    AsyncConnectionPool { pool, semaphore }
}

/// Runs embedded migrations on the database. Used to update database for end-users who don't build server themselves.
pub fn run_database_migrations(
    connection_pool: &AsyncConnectionPool,
) -> Result<RangeInclusive<i32>, Box<dyn Error + Send + Sync>> {
    let mut conn = connection_pool.get_blocking()?;
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

/// Runs other server-related migrations, like restructuring data folder or recomputing signatures
pub fn run_server_migrations(
    state: &AppState,
    migration_range: RangeInclusive<i32>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // If creating the database for the first time, set post signature version
    let mut conn = state.get_connection_blocking()?;
    if migration_range.contains(&1) {
        diesel::update(database_statistics::table)
            .set(database_statistics::signature_version.eq(SIGNATURE_VERSION))
            .execute(&mut conn)?;

        return Ok(());
    }

    // Update filenames if migrating primary keys to BIGINT
    if migration_range.contains(&12) {
        admin::database::reset_filenames_impl(state)?;
    }

    // Cache thumbnail sizes if migrating to statistics system
    if migration_range.contains(&13) {
        admin::database::reset_thumbnail_sizes_impl(state)?;
    }

    // Migrate to new post storage structure and fix checksum bug
    if migration_range.contains(&21) {
        admin::database::reset_filenames_impl(state)?;
        admin::post::recompute_checksums(state, &mut admin::mock_editor());
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

    let task: &str = AdminTask::RecomputeSignatures.into();
    error!(
        "Post signatures are out of date and need to be recomputed.

        This can be done via the admin CLI, which can be entered by passing
        the --admin flag to the server executable. If you are deploying with
        docker, you can do this by navigating to the source directory and
        executing the following command:
        
           docker exec -it oxibooru-server-1 ./server --admin
            
        While in the admin CLI, simply run the {task} task on all posts.
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
            diesel::sql_query("SET SESSION auto_explain.log_analyze = TRUE;").execute(conn)?;
        }
        Ok(())
    }

    fn on_release(&self, _conn: PgConnection) {}
}
