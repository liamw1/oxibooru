use crate::api::error::{ApiError, ApiResult};
use crate::app::AppState;
use crate::auth::Client;
use crate::config::{Config, RegexType};
use crate::model::enums::{Rating, UserRank};
use crate::string::SmallString;
use crate::time::DateTime;
use axum::Router;
use axum::http::StatusCode;
use serde::{Deserialize, Deserializer, Serialize};
use std::num::NonZero;
use std::ops::Deref;
use std::time::Duration;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

mod comment;
pub mod error;
mod extract;
mod info;
pub mod middleware;
mod password_reset;
mod pool;
mod pool_category;
mod post;
mod snapshot;
mod tag;
mod tag_category;
mod upload;
mod user;
mod user_token;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .merge(comment::routes())
        .merge(info::routes())
        .merge(password_reset::routes())
        .merge(pool_category::routes())
        .merge(pool::routes())
        .merge(post::routes())
        .merge(snapshot::routes())
        .merge(tag_category::routes())
        .merge(tag::routes())
        .merge(upload::routes())
        .merge(user_token::routes())
        .merge(user::routes())
        .layer((
            TraceLayer::new_for_http(),
            // Graceful shutdown will wait for outstanding requests to complete.
            // Add a timeout so requests don't hang forever.
            TimeoutLayer::new(Duration::from_secs(60)),
        ))
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), middleware::auth))
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), middleware::post_to_webhooks))
        .with_state(state)
        .fallback(|| async { (StatusCode::NOT_FOUND, "Route not found") })
}

/// Checks if the `client` is at least `required_rank`.
/// Returns error if client is lower rank than `required_rank`.
pub fn verify_privilege(client: Client, required_rank: UserRank) -> ApiResult<()> {
    (client.rank >= required_rank)
        .then_some(())
        .ok_or(ApiError::InsufficientPrivileges)
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

/// Represents body of a request to apply/change a score.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RatingBody {
    score: Rating,
}

impl Deref for RatingBody {
    type Target = Rating;
    fn deref(&self) -> &Self::Target {
        &self.score
    }
}

/// Represents body of a request to delete a resource.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeleteBody {
    version: DateTime,
}

impl Deref for DeleteBody {
    type Target = DateTime;
    fn deref(&self) -> &Self::Target {
        &self.version
    }
}

/// Represents body of a request to merge two resources.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MergeBody<T> {
    remove: T,
    merge_to: T,
    remove_version: DateTime,
    merge_to_version: DateTime,
}

/// Represents parameters of a request to retrieve one or more resources.
#[derive(Deserialize)]
struct ResourceParams {
    query: Option<String>,
    fields: Option<String>,
}

impl ResourceParams {
    fn criteria(&self) -> &str {
        self.query.as_deref().unwrap_or("")
    }

    fn fields(&self) -> Option<&str> {
        self.fields.as_deref()
    }
}

/// Represents parameters of a request to retrieve multiple resources, paged.
#[derive(Deserialize)]
struct PageParams {
    offset: Option<i64>,
    limit: NonZero<i64>,
    #[serde(flatten)]
    params: ResourceParams,
}

impl PageParams {
    fn criteria(&self) -> &str {
        self.params.criteria()
    }

    fn fields(&self) -> Option<&str> {
        self.params.fields()
    }

    fn into_query(self) -> Option<String> {
        self.params.query
    }
}

/// Represents a response to a request to retrieve multiple resources.
/// Used for resources which are not paged.
#[derive(Serialize)]
struct UnpagedResponse<T> {
    results: Vec<T>,
}

/// Represents a response to a request to retrieve multiple resources.
/// Used for resources which are paged.
#[derive(Serialize)]
struct PagedResponse<T> {
    query: Option<String>,
    offset: i64,
    limit: i64,
    total: i64,
    results: Vec<T>,
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
