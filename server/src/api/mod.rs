use crate::api::doc::ApiDoc;
use crate::api::error::{ApiError, ApiResult};
use crate::app::AppState;
use crate::auth::Client;
use crate::config::{Config, RegexType};
use crate::model::enums::{Rating, UserRank};
use crate::string::SmallString;
use crate::time::DateTime;
use axum::http::StatusCode;
use serde::{Deserialize, Deserializer, Serialize};
use std::num::NonZeroI64;
use std::ops::Deref;
use std::time::Duration;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use utoipa::{IntoParams, OpenApi, ToSchema};
use utoipa_axum::router::OpenApiRouter;

mod comment;
mod doc;
pub mod error;
mod extract;
mod info;
mod legacy;
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

/// Request body to apply/change a score.
#[derive(Deserialize, ToSchema)]
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

/// Request body for deleting a resource.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct DeleteBody {
    /// Resource version. See [versioning](#Versioning).
    version: DateTime,
}

impl Deref for DeleteBody {
    type Target = DateTime;
    fn deref(&self) -> &Self::Target {
        &self.version
    }
}

/// Request body for merging resources.
#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct MergeBody<T> {
    /// ID of the source resource to be removed.
    remove: T,
    /// ID of the target resource to merge into.
    merge_to: T,
    /// Version of the source resource. See [versioning](#Versioning).
    remove_version: DateTime,
    /// Version of the target resource. See [versioning](#Versioning).
    merge_to_version: DateTime,
}

/// Represents parameters of a request to retrieve one or more resources.
#[derive(Deserialize, IntoParams)]
struct ResourceParams {
    /// Query search string
    #[param(example = "anonymous_token")]
    query: Option<String>,
    /// Comma-separated list of fields to include in the response. See [field selection](#Field-Selection) for details.
    #[param(example = "field1,field2")]
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
#[derive(Deserialize, IntoParams)]
struct PageParams {
    /// Starting position in the result set
    #[param(example = 0)]
    offset: Option<i64>,
    /// Maximum number of results to return
    #[param(value_type = i64, minimum = 1, example = 40)]
    limit: NonZeroI64,
}

/// A result of search operation that doesn't involve paging.
#[derive(Serialize, ToSchema)]
struct UnpagedResponse<T> {
    results: Vec<T>,
}

/// A result of search operation that involves paging.
#[derive(Serialize, ToSchema)]
struct PagedResponse<T> {
    /// The query passed in the original request that contains a standard [search query](#Search).
    #[schema(examples("anonymous_token named_token:value1,value2,value3 sort:sort_token"))]
    query: Option<String>,
    /// The record starting offset, passed in the original request.
    #[schema(examples(0))]
    offset: i64,
    /// Number of records on one page.
    #[schema(examples(40))]
    limit: i64,
    /// How many resources were found. To get the page count, divide this number by `limit`.
    #[schema(examples(1729))]
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
