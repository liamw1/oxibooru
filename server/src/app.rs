use crate::api::middleware;
use crate::autotag::AutoTagSession;
use crate::config::Config;
use crate::content::cache::RingCache;
use crate::db::{ConnectionPool, ConnectionResult};
use crate::{admin, api, config, db, filesystem};
use axum::extract::Request;
use axum::{Router, ServiceExt};
use std::error::Error;
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::net::TcpListener;
use tokio::runtime::Handle;
use tokio::signal::unix::SignalKind;
use tower::ServiceBuilder;
use tower_http::normalize_path::NormalizePathLayer;
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use utoipa_swagger_ui::SwaggerUi;

#[derive(Clone)]
pub struct AppState {
    pub connection_pool: ConnectionPool,
    pub config: Arc<Config>,
    pub content_cache: Arc<Mutex<RingCache>>,
    pub auto_tag_session: Arc<Option<AutoTagSession>>,
}

impl AppState {
    pub fn new(connection_pool: ConnectionPool, config: Config, auto_tagging: Option<AutoTagSession>) -> Self {
        /// Max number of elements in the content cache. Should be as large as the number of users expected to be uploading concurrently.
        const CONTENT_CACHE_SIZE: usize = 10;
        AppState {
            connection_pool,
            config: Arc::new(config),
            content_cache: Arc::new(Mutex::new(RingCache::new(CONTENT_CACHE_SIZE))),
            auto_tag_session: Arc::new(auto_tagging),
        }
    }

    pub fn get_connection(&self) -> ConnectionResult {
        self.connection_pool.get()
    }

    pub fn get_content_cache(&self) -> MutexGuard<'_, RingCache> {
        match self.content_cache.lock() {
            Ok(guard) => guard,
            Err(err) => {
                error!("Content cache has been poisoned! Resetting...");
                let mut guard = err.into_inner();
                guard.clear();
                guard
            }
        }
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
pub fn enable_tracing(config: &Config) {
    let filter = match EnvFilter::try_new(&config.log_filter) {
        Ok(filter) => filter,
        Err(err) => {
            warn!("Log filter is invalid. Some or all directives may be ignored. Details:\n{err}");
            EnvFilter::new(&config.log_filter)
        }
    };
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().without_time())
        .init();
}

pub fn initialize(state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
    let migration_range = db::run_database_migrations(&state.connection_pool)?;
    db::run_server_migrations(state, migration_range)?;

    if admin::enabled() {
        admin::command_line_mode(state);
        std::process::exit(0);
    }

    let mut conn = state.get_connection()?;
    db::check_signature_version(&mut conn)?; // We do this after admin mode check so that users can update signatures
    middleware::initialize_snapshot_counter(&mut conn)?;

    if let Err(err) = filesystem::purge_temporary_uploads(&state.config) {
        warn!("Failed to purge temporary files. Details:\n{err}");
    }
    Ok(())
}

pub async fn run(state: AppState) -> std::io::Result<()> {
    let (router, api) = api::routes(state).split_for_parts();
    let normalized_router = ServiceBuilder::new()
        .layer(NormalizePathLayer::trim_trailing_slash())
        .service(router);
    let app = Router::new()
        .merge(SwaggerUi::new("/docs").url("/apidoc/openapi.json", api))
        .fallback_service(normalized_router);

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
