use crate::api::error::{ApiError, ApiResult};
use crate::api::middleware;
use crate::auth::Client;
use crate::config::{Action, Config};
use crate::content::cache::RingCache;
use crate::db::AsyncConnectionPool;
use crate::extract::Ctx;
use crate::{admin, api, config, db, filesystem, web};
use axum::Router;
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
    pub connection_pool: Arc<AsyncConnectionPool>,
    pub config: Arc<Config>,
    pub content_cache: Arc<Mutex<RingCache>>,
}

impl AppState {
    pub fn new(connection_pool: AsyncConnectionPool, config: Config) -> Self {
        /// Max number of elements in the content cache. Should be as large as the number of users expected to be uploading concurrently.
        const CONTENT_CACHE_SIZE: usize = 10;
        Self {
            connection_pool: Arc::new(connection_pool),
            config: Arc::new(config),
            content_cache: Arc::new(Mutex::new(RingCache::new(CONTENT_CACHE_SIZE))),
        }
    }

    pub fn make_context(self, client: Client) -> Ctx {
        Ctx(
            Context {
                client,
                config: self.config,
                content_cache: self.content_cache,
            },
            self.connection_pool,
        )
    }
}

#[derive(Clone)]
pub struct Context {
    pub client: Client,
    pub config: Arc<Config>,
    pub content_cache: Arc<Mutex<RingCache>>,
}

impl Context {
    pub fn site_name(&self) -> &str {
        &self.config.public_info.name
    }

    pub fn full_url(&self, relative_url: &str) -> String {
        let domain = if let Some(domain) = self.config.domain.as_ref() {
            domain.to_string()
        } else if let Ok(domain) = std::env::var("HTTP_ORIGIN") {
            domain
        } else if let Ok(domain) = std::env::var("HTTP_REFERER") {
            domain
        } else if let Ok(port) = std::env::var("PORT") {
            format!("http://localhost:{port}")
        } else {
            String::new()
        };

        let domain = domain.trim_end_matches('/');
        let relative_url = relative_url.trim_start_matches('/');
        format!("{domain}/{relative_url}")
    }

    pub fn safety_enabled(&self) -> bool {
        self.config.public_info.enable_safety
    }

    /// Checks if the `client` is at least `required_rank`.
    pub fn has_privilege(&self, action: Action) -> bool {
        self.client.rank >= self.config.privileges()[action]
    }

    pub fn can_edit_posts(&self) -> bool {
        self.has_privilege(Action::PostEditContent)
            || self.has_privilege(Action::PostEditDescription)
            || self.has_privilege(Action::PostEditFlag)
            || self.has_privilege(Action::PostEditNote)
            || self.has_privilege(Action::PostEditRelation)
            || self.has_privilege(Action::PostEditSafety)
            || self.has_privilege(Action::PostEditSource)
            || self.has_privilege(Action::PostEditTag)
            || self.has_privilege(Action::PostEditThumbnail)
            || self.has_privilege(Action::PostFeature)
            || self.has_privilege(Action::PostDelete)
            || self.has_privilege(Action::PostMerge)
    }

    pub fn can_edit_tag_categories(&self) -> bool {
        self.has_privilege(Action::TagCategoryCreate)
            || self.has_privilege(Action::TagCategoryEditColor)
            || self.has_privilege(Action::TagCategoryEditName)
            || self.has_privilege(Action::TagCategoryEditOrder)
            || self.has_privilege(Action::TagCategorySetDefault)
            || self.has_privilege(Action::TagCategoryDelete)
    }

    /// Returns error if client is lower rank than `required_rank`.
    pub fn verify_privilege(&self, action: Action) -> ApiResult<()> {
        self.has_privilege(action)
            .then_some(())
            .ok_or(ApiError::InsufficientPrivileges)
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
    std::thread::available_parallelism().map_or(1, |threads| std::cmp::max(threads.get() / 2, 1))
}

/// Initializes logging using [`tracing_subscriber`].
pub fn enable_tracing(state: &AppState) {
    let filter = match EnvFilter::try_new(&state.config.log_filter) {
        Ok(filter) => filter,
        Err(err) => {
            warn!("Log filter is invalid. Some or all directives may be ignored. Details:\n{err}");
            EnvFilter::new(&state.config.log_filter)
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

    let mut conn = state.connection_pool.get_blocking()?;
    db::check_signature_version(&mut conn)?; // We do this after admin mode check so that users can update signatures
    middleware::initialize_snapshot_counter(&mut conn)?;

    if let Err(err) = filesystem::purge_temporary_uploads(&state.config) {
        warn!("Failed to purge temporary files. Details:\n{err}");
    }
    filesystem::spawn_temporary_uploads_cleanup_task(Arc::clone(&state.config));
    Ok(())
}

pub async fn run(state: AppState) -> std::io::Result<()> {
    let (api_router, open_api) = api::routes(state.clone()).split_for_parts();
    let normalized_api_router = ServiceBuilder::new()
        .layer(NormalizePathLayer::trim_trailing_slash())
        .service(api_router);
    let api_router = Router::new()
        .merge(SwaggerUi::new("/docs").url("/apidoc/openapi.json", open_api))
        .fallback_service(normalized_api_router);
    let app = web::routes(state).nest("/api", api_router);

    let address = format!("0.0.0.0:{}", config::port());
    let listener = TcpListener::bind(address).await?;
    info!("Oxibooru server running on {} threads", Handle::current().metrics().num_workers());
    debug!("listening on {}", listener.local_addr()?);
    axum::serve(listener, app)
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
