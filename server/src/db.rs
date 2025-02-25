use crate::admin::{database, AdminTask};
use crate::config;
use crate::content::signature::SIGNATURE_VERSION;
use crate::schema::database_statistics;
#[cfg(test)]
use crate::test;
use diesel::migration::Migration;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool, PoolError, PooledConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use std::borrow::Cow;
use std::sync::LazyLock;
use std::time::Duration;

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
pub fn run_migrations(conn: &mut PgConnection) {
    let pending_migrations = conn.pending_migrations(MIGRATIONS).unwrap();
    if pending_migrations.is_empty() {
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

    println!("Running pending migrations...");
    conn.run_pending_migrations(MIGRATIONS).unwrap();
    if cfg!(test) {
        return;
    }

    // If creating the database for the first time, set post signature version
    if migration_range.contains(&1) {
        diesel::update(database_statistics::table)
            .set(database_statistics::signature_version.eq(SIGNATURE_VERSION))
            .execute(conn)
            .unwrap();
    }

    // Cache thumbnail sizes if migrating to statistics system
    if migration_range.contains(&13) {
        database::reset_thumbnail_sizes(conn).unwrap();
    }
}

/// Returns a url for the database using `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_HOST`, and `POSTGRES_DATABASE`
/// environment variables. If `database_override` is not `None`, then it's value will be used in place of `POSTGRES_DATABASE`.
pub fn create_url(database_override: Option<&str>) -> String {
    if std::env::var("DOCKER_DEPLOYMENT").is_err() {
        dotenvy::from_filename("../.env").unwrap();
    }

    let user = std::env::var("POSTGRES_USER").unwrap();
    let password = std::env::var("POSTGRES_PASSWORD").unwrap();
    let hostname = std::env::var("POSTGRES_HOST").unwrap_or_else(|_| String::from("localhost"));
    let database = database_override
        .map(Cow::Borrowed)
        .unwrap_or_else(|| Cow::Owned(std::env::var("POSTGRES_DB").unwrap()));

    format!("postgres://{user}:{password}@{hostname}/{database}")
}

pub fn check_signature_version() {
    let get_current_version = |conn: &mut PgConnection| -> i32 {
        database_statistics::table
            .select(database_statistics::signature_version)
            .first(conn)
            .unwrap()
    };

    let mut conn = get_connection().unwrap();
    let current_version = get_current_version(&mut conn);
    if current_version == SIGNATURE_VERSION {
        return;
    }

    let task: &str = AdminTask::RecomputePostSignatures.into();
    println!(
        "ERROR: Post signatures are out of date and need to be recomputed.

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
    while get_current_version(&mut conn) != SIGNATURE_VERSION {
        std::thread::sleep(Duration::from_millis(500));
    }
}

const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

static CONNECTION_POOL: LazyLock<ConnectionPool> = LazyLock::new(|| {
    let num_tokio_threads = tokio::runtime::Handle::try_current()
        .map(|handle| handle.metrics().num_workers())
        .unwrap_or(1);
    let num_rayon_threads = rayon::current_num_threads();
    let num_threads = std::cmp::max(num_tokio_threads, num_rayon_threads) as u32;

    let manager = ConnectionManager::new(config::database_url());
    Pool::builder()
        .max_size(num_threads)
        .max_lifetime(None)
        .idle_timeout(None)
        .test_on_check_out(true)
        .build(manager)
        .expect("Could not build connection pool")
});
