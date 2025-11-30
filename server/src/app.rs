use crate::api::middleware;
use crate::db::{ConnectionPool, ConnectionResult};
use crate::{admin, api, config, db, filesystem};
use axum::ServiceExt;
use axum::extract::Request;
use std::error::Error;
use tokio::net::TcpListener;
use tokio::runtime::Handle;
use tokio::signal::unix::SignalKind;
use tower::layer::Layer;
use tower_http::normalize_path::NormalizePathLayer;
use tracing::{debug, info, warn};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Clone, Debug)]
pub struct AppState {
    pub connection_pool: ConnectionPool,
}

impl AppState {
    pub fn new(connection_pool: ConnectionPool) -> Self {
        AppState { connection_pool }
    }

    pub fn get_connection(&self) -> ConnectionResult {
        self.connection_pool.get()
    }
}

/// Returns the number of threads that the global rayon thread pool will
/// be constructed with. The rayon thread pool is currently only used when
/// executing admin commands.
pub fn num_rayon_threads() -> usize {
    std::thread::available_parallelism()
        .map(|threads| std::cmp::max(threads.get() / 2, 1))
        .unwrap_or(1)
}

/// Initializes logging using [`tracing_subscriber`].
pub fn enable_tracing() {
    let filter = match EnvFilter::try_new(&config::get().log_filter) {
        Ok(filter) => filter,
        Err(err) => {
            warn!("Log filter is invalid. Some or all directives may be ignored. Details:\n{err}");
            EnvFilter::new(&config::get().log_filter)
        }
    };
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().without_time())
        .init();
}

pub fn initialize() -> Result<(), Box<dyn Error + Send + Sync>> {
    let connection_pool = db::create_connection_pool();
    let migration_range = db::run_database_migrations(&connection_pool)?;
    db::run_server_migrations(&connection_pool, migration_range)?;

    if admin::enabled() {
        admin::command_line_mode(&connection_pool);
        std::process::exit(0);
    }

    let mut conn = connection_pool.get()?;
    db::check_signature_version(&mut conn)?; // We do this after admin mode check so that users can update signatures
    middleware::initialize_snapshot_counter(&mut conn)?;

    if let Err(err) = filesystem::purge_temporary_uploads() {
        warn!("Failed to purge temporary files. Details:\n{err}");
    }
    Ok(())
}

pub async fn run() -> std::io::Result<()> {
    let state = AppState::new(db::create_connection_pool());
    let app = NormalizePathLayer::trim_trailing_slash().layer(api::routes(state));

    let address = format!("0.0.0.0:{}", config::port());
    let listener = TcpListener::bind(address).await?;
    info!("Oxibooru server running on {} threads", Handle::current().metrics().num_workers());
    debug!("listening on {}", listener.local_addr()?);
    axum::serve(listener, ServiceExt::<Request>::into_make_service(app))
        .with_graceful_shutdown(shutdown_signal())
        .await
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Ctrl+C handler must be installable");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(SignalKind::terminate())
            .expect("Signal handler must be installable")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    info!("Stopping server...");
}
