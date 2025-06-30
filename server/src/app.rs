use crate::{api, config};
use axum::ServiceExt;
use axum::extract::Request;
use tokio::net::TcpListener;
use tokio::runtime::Handle;
use tokio::signal::unix::SignalKind;
use tower::layer::Layer;
use tower_http::normalize_path::NormalizePathLayer;
use tracing::{debug, info};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub fn enable_tracing() {
    let log_filter = config::get()
        .log_filter
        .as_deref()
        .unwrap_or("server=info,tower_http=debug,axum=trace");
    tracing_subscriber::registry()
        .with(EnvFilter::try_new(log_filter).unwrap())
        .with(tracing_subscriber::fmt::layer().without_time())
        .init();
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
