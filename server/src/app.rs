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

    if let Err(err) = filesystem::purge_temporary_uploads() {
        warn!("Failed to purge temporary files. Details:\n\n{err}");
    }
    Ok(())
}

pub async fn run() {
    let app = NormalizePathLayer::trim_trailing_slash().layer(api::routes());

    let address = format!("0.0.0.0:{}", config::port());
    let listener = TcpListener::bind(address).await.unwrap();
    info!("Oxibooru server running on {} threads", Handle::current().metrics().num_workers());
    debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, ServiceExt::<Request>::into_make_service(app))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("Stopping server...")
}
