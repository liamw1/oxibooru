use crate::api::doc::ApiDoc;
use crate::api::error::{ApiError, ApiResult};
use crate::app::AppState;
use crate::auth::Client;
use crate::config::{Config, RegexType};
use crate::model::enums::UserRank;
use crate::string::SmallString;
use crate::time::DateTime;
use axum::http::StatusCode;
use serde::{Deserialize, Deserializer};
use std::time::Duration;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;

mod comment;
mod doc;
pub mod error;
pub mod info;
mod legacy;
pub mod middleware;
mod password_reset;
pub mod pool;
pub mod pool_category;
pub mod post;
mod snapshot;
pub mod tag;
pub mod tag_category;
mod upload;
mod user;
mod user_token;

pub fn routes(state: AppState) -> OpenApiRouter {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .merge(comment::routes())
        .merge(info::routes())
        .merge(legacy::routes())
        .merge(password_reset::routes())
        .merge(pool::routes())
        .merge(pool_category::routes())
        .merge(post::routes())
        .merge(snapshot::routes())
        .merge(tag::routes())
        .merge(tag_category::routes())
        .merge(upload::routes())
        .merge(user::routes())
        .merge(user_token::routes())
        .layer((
            TraceLayer::new_for_http(),
            TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_mins(1)),
        ))
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), middleware::auth))
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), middleware::post_to_webhooks))
        .route_layer(axum::middleware::from_fn(middleware::log_error))
        .with_state(state)
        .fallback(|| async { (StatusCode::NOT_FOUND, "Route not found") })
}

/// Checks if `haystack` matches regex `regex_type`.
/// Returns error if it does not match on the regex.
pub fn verify_matches_regex(config: &Config, haystack: &str, regex_type: RegexType) -> ApiResult<()> {
    config
        .regex(regex_type)
        .is_match(haystack)
        .then_some(())
        .ok_or_else(|| ApiError::ExpressionFailsRegex(SmallString::new(haystack), regex_type))
}

/// Checks if `email` is a valid email.
/// Returns error if `email` is invalid.
pub fn verify_valid_email(email: Option<&str>) -> Result<(), lettre::address::AddressError> {
    match email {
        Some(address) => address.parse::<lettre::Address>().map(|_| ()),
        None => Ok(()),
    }
}

/// Checks if the `client` is at least `required_rank`.
/// Returns error if client is lower rank than `required_rank`.
fn verify_privilege(client: Client, required_rank: UserRank) -> ApiResult<()> {
    (client.rank >= required_rank)
        .then_some(())
        .ok_or(ApiError::InsufficientPrivileges)
}

/// Checks if `current_version` matches `client_version`.
/// Returns error if they do not match.
fn verify_version(current_version: DateTime, client_version: DateTime) -> ApiResult<()> {
    // Check disabled in test builds
    if cfg!(test) {
        return Ok(());
    }

    (current_version == client_version)
        .then_some(())
        .ok_or(ApiError::ResourceModified)
}

// Any value that is present is considered Some value, including null.
fn deserialize_some<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}
