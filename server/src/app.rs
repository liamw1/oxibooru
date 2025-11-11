use crate::api::middleware;
use crate::{admin, api, config, db, filesystem};
use axum::ServiceExt;
use axum::extract::Request;
use tokio::net::TcpListener;
use tokio::runtime::Handle;
use tokio::signal::unix::SignalKind;
use tower::layer::Layer;
use tower_http::normalize_path::NormalizePathLayer;
use tracing::{debug, info, warn};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Returns the number of threads that the global rayon thread pool will
/// be constructed with. The rayon thread pool is currently only used when
/// executing admin commands.
pub fn num_rayon_threads() -> usize {
    std::thread::available_parallelism()
        .map(|threads| std::cmp::max(threads.get() / 2, 1))
        .unwrap_or(1)
}

pub fn enable_tracing() {
    const DEFAULT_DIRECTIVE: &str = "server=info,tower_http=debug,axum=trace";
    let directive = config::get().log_filter.as_deref().unwrap_or(DEFAULT_DIRECTIVE);
    let filter = EnvFilter::try_new(directive).unwrap_or(EnvFilter::new(DEFAULT_DIRECTIVE));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().without_time())
        .init();
}

pub fn initialize() -> Result<(), String> {
    let mut conn = db::get_connection().map_err(|err| err.to_string())?;
    db::run_migrations(&mut conn).map_err(|err| err.to_string())?;

    if admin::enabled() {
        admin::command_line_mode(&mut conn);
        std::process::exit(0);
    }

    // We do this after admin mode check so that users can update signatures
    db::check_signature_version(&mut conn).map_err(|err| err.to_string())?;

    middleware::initialize_snapshot_counter(&mut conn).map_err(|err| err.to_string())?;

    if let Err(err) = filesystem::purge_temporary_uploads() {
        warn!("Failed to purge temporary files. Details:\n{err}");
    }
    Ok(())
}

pub async fn run() -> std::io::Result<()> {
    let app = NormalizePathLayer::trim_trailing_slash().layer(api::routes());

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
