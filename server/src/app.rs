use crate::api::error::{ApiError, ApiResult};
use crate::api::middleware;
use crate::auth::Client;
use crate::config::{Action, Config, Env};
use crate::content::cache::RingCache;
use crate::db::AsyncConnectionPool;
use crate::extract::Ctx;
use crate::model::enums::UserRank;
use crate::search::preferences::Preferences;
use crate::{admin, api, db, filesystem, web};
use axum::Router;
use reqwest::Client as HttpClient;
use std::error::Error;
use std::fmt::Display;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use tokio::net::TcpListener;
use tokio::runtime::Handle;
#[cfg(unix)]
use tokio::signal::unix::SignalKind;
use tower_http::normalize_path::NormalizePathLayer;
use tower_layer::Layer;
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use utoipa_swagger_ui::SwaggerUi;

#[derive(Clone)]
pub struct AppState {
    pub env: Arc<Env>,
    pub config: Arc<Config>,
    pub downloader: HttpClient,
    pub connection_pool: AsyncConnectionPool,
    pub content_cache: Arc<Mutex<RingCache>>,
}

impl AppState {
    pub fn new(
        downloader: HttpClient,
        connection_pool: AsyncConnectionPool,
        env: Arc<Env>,
        config: Arc<Config>,
    ) -> Self {
        /// Max number of elements in the content cache. Should be roughly as large as the number of users expected to be uploading concurrently.
        const CONTENT_CACHE_SIZE: usize = 10;
        Self {
            env,
            config,
            downloader,
            connection_pool,
            content_cache: Arc::new(Mutex::new(RingCache::new(CONTENT_CACHE_SIZE))),
        }
    }

    pub fn make_context(self, client: Client) -> Ctx {
        Ctx(
            Context {
                client,
                env: self.env,
                config: self.config,
                downloader: self.downloader,
                content_cache: self.content_cache,
            },
            self.connection_pool,
        )
    }
}

#[derive(Clone)]
pub struct Context {
    pub env: Arc<Env>,
    pub config: Arc<Config>,
    pub client: Client,
    pub downloader: HttpClient,
    pub content_cache: Arc<Mutex<RingCache>>,
}

impl Context {
    pub fn site_name(&self) -> &str {
        &self.config.public_info.name
    }

    pub fn full_url(&self, relative_url: &str) -> String {
        let domain = if let Some(domain) = self.config.domain.as_deref() {
            domain
        } else if let Some(domain) = self.env.http_origin.as_deref() {
            domain
        } else if let Some(domain) = self.env.http_referer.as_deref() {
            domain
        } else if let Some(port) = self.env.domain_port {
            &format!("http://localhost:{port}")
        } else {
            ""
        };

        let domain = domain.trim_end_matches('/');
        let relative_url = relative_url.trim_start_matches('/');
        format!("{domain}/{relative_url}")
    }

    pub fn safety_enabled(&self) -> bool {
        self.config.public_info.enable_safety
    }

    /// Checks if the client can perform given `action`.
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

    pub fn can_edit_pools(&self) -> bool {
        self.has_privilege(Action::PoolEditName)
            | self.has_privilege(Action::PoolEditCategory)
            | self.has_privilege(Action::PoolEditDescription)
            | self.has_privilege(Action::PoolEditPost)
    }

    pub fn can_edit_pool_categories(&self) -> bool {
        self.has_privilege(Action::PoolCategoryCreate)
            || self.has_privilege(Action::PoolCategoryEditName)
            || self.has_privilege(Action::PoolCategoryEditColor)
            || self.has_privilege(Action::PoolCategoryDelete)
            || self.has_privilege(Action::PoolCategorySetDefault)
    }

    pub fn can_edit_tags(&self) -> bool {
        self.has_privilege(Action::TagEditName)
            | self.has_privilege(Action::TagEditCategory)
            | self.has_privilege(Action::TagEditDescription)
            | self.has_privilege(Action::TagEditImplication)
            | self.has_privilege(Action::TagEditSuggestion)
    }

    pub fn can_edit_tag_categories(&self) -> bool {
        self.has_privilege(Action::TagCategoryCreate)
            || self.has_privilege(Action::TagCategoryEditName)
            || self.has_privilege(Action::TagCategoryEditColor)
            || self.has_privilege(Action::TagCategoryEditOrder)
            || self.has_privilege(Action::TagCategoryDelete)
            || self.has_privilege(Action::TagCategorySetDefault)
    }

    /// Returns error if client cannot perform given `action`.
    pub fn verify_privilege(&self, action: Action) -> ApiResult<()> {
        self.has_privilege(action)
            .then_some(())
            .ok_or(ApiError::InsufficientPrivileges)
    }

    pub fn get_content_cache(&self) -> MutexGuard<'_, RingCache> {
        self.content_cache.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn preferences(&self) -> &Preferences {
        match self.client.rank {
            UserRank::Anonymous => &self.config.anonymous_preferences,
            UserRank::Restricted => &self.config.restricted_preferences,
            UserRank::Regular => &self.config.regular_preferences,
            UserRank::Power => &self.config.power_preferences,
            UserRank::Moderator => &self.config.moderator_preferences,
            UserRank::Administrator => &self.config.administrator_preferences,
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
pub fn enable_tracing(config: &Config) {
    let initialize = |filter: EnvFilter| {
        tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer().without_time())
            .init();
    };
    match EnvFilter::try_new(&config.log_filter) {
        Ok(filter) => initialize(filter),
        Err(err) => {
            initialize(EnvFilter::new(&config.log_filter));
            warn!("Log filter is invalid. Some or all directives may be ignored. Details:\n{err}");
        }
    }
}

/// Runs database migrations, initializes snapshot counter, and spawns any long-running tasks.
pub fn initialize(state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(migration_range) = db::run_database_migrations(&state.connection_pool)? {
        db::run_server_migrations(state, migration_range)?;
    }

    if state.config.args.admin_mode {
        admin::command_line_mode(state);
        std::process::exit(0);
    }

    let mut conn = state.connection_pool.get_blocking()?;
    db::check_signature_version(&mut conn)?; // We do this after admin mode check so that users can update signatures
    middleware::initialize_snapshot_counter(&mut conn)?;
    filesystem::spawn_temporary_uploads_cleanup_task(Arc::clone(&state.config));
    Ok(())
}

pub async fn run(state: AppState) -> std::io::Result<()> {
    let server_port = state.env.server_port;

    let (api_router, open_api) = api::routes(state.clone()).split_for_parts();
    let api_router = Router::new()
        .merge(SwaggerUi::new("/docs").url("/apidoc/openapi.json", open_api))
        .fallback_service(NormalizePathLayer::trim_trailing_slash().layer(api_router));
    let app = web::routes(state).nest("/api", api_router);

    let address = format!("0.0.0.0:{server_port}");
    let listener = TcpListener::bind(address).await?;
    info!("Oxibooru server running on {} threads", Handle::current().metrics().num_workers());
    debug!("listening on {}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
}

/// Logs given `message` and `error` to console and terminates the process.
pub fn shutdown<E: Display>(message: &str, error: E) -> ! {
    error!("{message}. Details:\n{error}");
    std::process::exit(1)
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
