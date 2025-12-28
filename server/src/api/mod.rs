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
use utoipa::{OpenApi, ToSchema};
use utoipa_axum::router::OpenApiRouter;

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

pub fn routes(state: AppState) -> OpenApiRouter {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .merge(comment::routes())
        .merge(info::routes())
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

const COMMENT_TAG: &str = "comment";
const INFO_TAG: &str = "info";
const PASSWORD_RESET_TAG: &str = "password_reset";
const POOL_TAG: &str = "pool";
const POOL_CATEGORY_TAG: &str = "pool_category";
const POST_TAG: &str = "post";
const SNAPSHOT_TAG: &str = "snapshot";
const TAG_TAG: &str = "tag";
const TAG_CATEGORY_TAG: &str = "tag_category";
const UPLOAD_TAG: &str = "upload";
const USER_TAG: &str = "user";
const USER_TOKEN_TAG: &str = "user_token";

#[derive(OpenApi)]
#[openapi(
    tags(
        (name = COMMENT_TAG, description = "Comment API endpoints"),
        (name = INFO_TAG, description = "Info API endpoints"),
        (name = PASSWORD_RESET_TAG, description = "Password reset API endpoints"),
        (name = POOL_TAG, description = "Pool API endpoints"),
        (name = POOL_CATEGORY_TAG, description = "Pool category API endpoints"),
        (name = POST_TAG, description = "Post API endpoints"),
        (name = SNAPSHOT_TAG, description = "Snapshot API endpoints"),
        (name = TAG_TAG, description = "Tag API endpoints"),
        (name = TAG_CATEGORY_TAG, description = "Tag category API endpoints"),
        (name = UPLOAD_TAG, description = "Upload API endpoints"),
        (name = USER_TAG, description = "User API endpoints"),
        (name = USER_TOKEN_TAG, description = "User token API endpoints"),
    )
)]
struct ApiDoc;

/// Represents body of a request to apply/change a score.
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

/// Represents body of a request to delete a resource.
#[derive(Deserialize, ToSchema)]
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
#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct MergeBody<T> {
    remove: T,
    merge_to: T,
    remove_version: DateTime,
    merge_to_version: DateTime,
}

/// Represents parameters of a request to retrieve one or more resources.
#[derive(Deserialize, ToSchema)]
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
#[derive(Deserialize, ToSchema)]
struct PageParams {
    offset: Option<i64>,
    #[schema(value_type = i64)]
    limit: NonZeroI64,
    #[schema(inline)]
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
#[derive(Serialize, ToSchema)]
struct UnpagedResponse<T> {
    results: Vec<T>,
}

/// Represents a response to a request to retrieve multiple resources.
/// Used for resources which are paged.
#[derive(Serialize, ToSchema)]
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
