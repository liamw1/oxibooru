mod comment;
mod info;
mod password_reset;
mod pool;
mod pool_category;
mod post;
mod tag;
mod tag_category;
mod upload;
mod user;
mod user_token;

use crate::auth::header::{self, AuthUser, AuthenticationError};
use crate::config::RegexType;
use crate::error::ErrorKind;
use crate::model::enums::{MimeType, Rating, ResourceType, UserRank};
use crate::time::DateTime;
use crate::{config, update};
use serde::{Deserialize, Deserializer, Serialize};
use std::convert::Infallible;
use std::num::NonZero;
use std::ops::Deref;
use warp::http::StatusCode;
use warp::reply::{Json, Response, WithStatus};
use warp::{Filter, Rejection};

pub type ApiResult<T> = Result<T, Error>;

pub enum Reply {
    Json(Json),
    WithStatus(WithStatus<Json>),
}

impl warp::Reply for Reply {
    fn into_response(self) -> Response {
        match self {
            Self::Json(reply) => reply.into_response(),
            Self::WithStatus(reply) => reply.into_response(),
        }
    }
}

impl<T: Serialize> From<ApiResult<T>> for Reply {
    fn from(value: ApiResult<T>) -> Self {
        match value {
            Ok(response) => Self::Json(warp::reply::json(&response)),
            Err(err) => {
                eprintln!("{}: {err}", err.kind());
                let response = warp::reply::json(&err.response());
                Self::WithStatus(warp::reply::with_status(response, err.status_code()))
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub enum Error {
    BadExtension(#[from] crate::model::enums::ParseExtensionError),
    BadHash(#[from] crate::auth::HashError),
    BadHeader(#[from] warp::http::header::ToStrError),
    #[error("File of type {0} did not match request with content-type {1}")]
    ContentTypeMismatch(MimeType, String),
    #[error("Cyclic dependency detected in {0}s")]
    CyclicDependency(ResourceType),
    #[error("Cannot delete default {0}")]
    DeleteDefault(ResourceType),
    #[error("Expression does not match on {0} regex")]
    ExpressionFailsRegex(RegexType),
    FailedAuthentication(#[from] AuthenticationError),
    FailedConnection(#[from] diesel::r2d2::PoolError),
    FailedEmailTransport(#[from] lettre::transport::smtp::Error),
    FailedQuery(#[from] diesel::result::Error),
    FromStr(#[from] Box<dyn std::error::Error>),
    #[error("Insufficient privileges")]
    InsufficientPrivileges,
    InvalidEmailAddress(#[from] lettre::address::AddressError),
    InvalidEmail(#[from] lettre::error::Error),
    #[error("Metadata must be application/json")]
    InvalidMetadataType,
    #[error("Cannot create an anonymous user")]
    InvalidUserRank,
    Image(#[from] image::ImageError),
    JsonSerialization(#[from] serde_json::Error),
    #[error("Form is missing content-type")]
    MissingContentType,
    #[error("Missing form data")]
    MissingFormData,
    #[error("Missing metadata form")]
    MissingMetadata,
    #[error("Missing smtp info")]
    MissingSmtpInfo,
    #[error("User has no email")]
    NoEmail,
    #[error("{0} needs at least one name")]
    NoNamesGiven(ResourceType),
    NotAnInteger(#[from] std::num::ParseIntError),
    #[error("{0} not found")]
    NotFound(ResourceType),
    #[error("This action requires you to be logged in")]
    NotLoggedIn,
    #[error("Someone else modified this in the meantime. Please try again.")]
    ResourceModified,
    Search(#[from] crate::search::Error),
    #[error("Cannot merge {0} with itself")]
    SelfMerge(ResourceType),
    StdIo(#[from] std::io::Error),
    #[error("Password reset token is invalid")]
    UnauthorizedPasswordReset,
    Utf8Conversion(#[from] std::str::Utf8Error),
    VideoDecoding(#[from] video_rs::Error),
    Warp(#[from] warp::Error),
}

impl Error {
    fn status_code(&self) -> StatusCode {
        use serde_json::error::Category;
        type QueryError = diesel::result::Error;

        let query_error_status_code = |err: &QueryError| match err {
            QueryError::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        match self {
            Self::BadExtension(_) => StatusCode::BAD_REQUEST,
            Self::BadHash(_) => StatusCode::BAD_REQUEST,
            Self::BadHeader(_) => StatusCode::BAD_REQUEST,
            Self::ContentTypeMismatch(..) => StatusCode::BAD_REQUEST,
            Self::CyclicDependency(_) => StatusCode::BAD_REQUEST,
            Self::DeleteDefault(_) => StatusCode::BAD_REQUEST,
            Self::ExpressionFailsRegex(_) => StatusCode::BAD_GATEWAY,
            Self::FailedAuthentication(err) => match err {
                AuthenticationError::FailedConnection(_) => StatusCode::SERVICE_UNAVAILABLE,
                AuthenticationError::FailedQuery(err) => query_error_status_code(err),
                _ => StatusCode::UNAUTHORIZED,
            },
            Self::FailedConnection(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::FailedEmailTransport(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::FailedQuery(err) => query_error_status_code(err),
            Self::FromStr(_) => StatusCode::BAD_REQUEST,
            Self::InsufficientPrivileges => StatusCode::FORBIDDEN,
            Self::InvalidEmailAddress(_) => StatusCode::BAD_REQUEST,
            Self::InvalidEmail(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::InvalidMetadataType => StatusCode::BAD_REQUEST,
            Self::InvalidUserRank => StatusCode::BAD_REQUEST,
            Self::Image(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::JsonSerialization(err) => match err.classify() {
                Category::Io | Category::Eof => StatusCode::INTERNAL_SERVER_ERROR,
                Category::Syntax | Category::Data => StatusCode::BAD_REQUEST,
            },
            Self::MissingContentType => StatusCode::BAD_REQUEST,
            Self::MissingFormData => StatusCode::BAD_REQUEST,
            Self::MissingMetadata => StatusCode::BAD_REQUEST,
            Self::MissingSmtpInfo => StatusCode::INTERNAL_SERVER_ERROR,
            Self::NoEmail => StatusCode::BAD_REQUEST,
            Self::NoNamesGiven(_) => StatusCode::BAD_REQUEST,
            Self::NotAnInteger(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::NotLoggedIn => StatusCode::FORBIDDEN,
            Self::ResourceModified => StatusCode::CONFLICT,
            Self::Search(_) => StatusCode::BAD_REQUEST,
            Self::SelfMerge(_) => StatusCode::BAD_REQUEST,
            Self::StdIo(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::UnauthorizedPasswordReset => StatusCode::UNAUTHORIZED,
            Self::Utf8Conversion(_) => StatusCode::BAD_REQUEST,
            Self::VideoDecoding(_) => StatusCode::BAD_REQUEST,
            Self::Warp(_) => StatusCode::BAD_REQUEST,
        }
    }

    fn category(&self) -> &'static str {
        match self {
            Self::BadExtension(_) => "Bad Extension",
            Self::BadHash(_) => "Bad Hash",
            Self::BadHeader(_) => "Bad Header",
            Self::ContentTypeMismatch(..) => "Content Type Mismatch",
            Self::CyclicDependency(_) => "Cyclic Dependency",
            Self::DeleteDefault(_) => "Delete Default",
            Self::ExpressionFailsRegex(_) => "Expression Fails Regex",
            Self::FailedAuthentication(_) => "Failed Authentication",
            Self::FailedConnection(_) => "Failed Connection",
            Self::FailedEmailTransport(_) => "Failed Email Transport",
            Self::FailedQuery(_) => "Failed Query",
            Self::FromStr(_) => "FromStr Error",
            Self::InsufficientPrivileges => "Insufficient Privileges",
            Self::InvalidEmailAddress(_) => "Invalid Email Address",
            Self::InvalidEmail(_) => "Invalid Email",
            Self::InvalidMetadataType => "Invalid Metadata Type",
            Self::InvalidUserRank => "Invalid User Rank",
            Self::Image(_) => "Image Error",
            Self::JsonSerialization(_) => "JSON Serialization Error",
            Self::MissingContentType => "Missing Content Type",
            Self::MissingFormData => "Missing Form Data",
            Self::MissingMetadata => "Missing Metadata",
            Self::MissingSmtpInfo => "Missing SMTP Info",
            Self::NoEmail => "No Email",
            Self::NoNamesGiven(_) => "No Names Given",
            Self::NotAnInteger(_) => "Parse Int Error",
            Self::NotFound(_) => "Resource Not Found",
            Self::NotLoggedIn => "Not Logged In",
            Self::ResourceModified => "Resource Modified",
            Self::Search(_) => "Search Error",
            Self::SelfMerge(_) => "Self Merge",
            Self::StdIo(_) => "IO Error",
            Self::UnauthorizedPasswordReset => "Unauthorized Password Reset",
            Self::Utf8Conversion(_) => "Utf8 Conversion Error",
            Self::VideoDecoding(_) => "Video Decoding Error",
            Self::Warp(_) => "Warp Error",
        }
    }

    fn response(&self) -> ErrorResponse {
        ErrorResponse {
            name: self.kind(),
            title: self.category(),
            description: self.to_string(),
        }
    }
}

/// Checks if the `client` is at least `required_rank`.
/// Returns error if client is lower rank than `required_rank`.
pub fn verify_privilege(client: Option<AuthUser>, required_rank: UserRank) -> ApiResult<()> {
    (client_access_level(client) >= required_rank)
        .then_some(())
        .ok_or(Error::InsufficientPrivileges)
}

/// Checks if `haystack` matches regex `regex_type`.
/// Returns error if it does not match on the regex.
pub fn verify_matches_regex(haystack: &str, regex_type: RegexType) -> ApiResult<()> {
    config::regex(regex_type)
        .is_match(haystack)
        .then_some(())
        .ok_or(Error::ExpressionFailsRegex(regex_type))
}

/// Checks if `email` is a valid email.
/// Returns error if `email` is invalid.
pub fn verify_valid_email(email: Option<&str>) -> Result<(), lettre::address::AddressError> {
    match email {
        Some(address) => address.parse::<lettre::Address>().map(|_| ()),
        None => Ok(()),
    }
}

/// Returns all possible routes for the application.
pub fn routes() -> impl Filter<Extract = impl warp::Reply, Error = Infallible> + Clone {
    let catch_all = warp::any().map(|| {
        eprintln!("No endpoint for request!");
        warp::reply::with_status("Bad Request", StatusCode::BAD_REQUEST)
    });
    let log = warp::filters::log::custom(|info| {
        println!("{} {} [{}]", info.method(), info.path(), info.status());
    });

    info::routes()
        .or(comment::routes())
        .or(password_reset::routes())
        .or(pool_category::routes())
        .or(pool::routes())
        .or(post::routes())
        .or(tag_category::routes())
        .or(tag::routes())
        .or(upload::routes())
        .or(user_token::routes())
        .or(user::routes())
        .or(catch_all)
        .with(log)
}

type AuthResult = Result<Option<AuthUser>, AuthenticationError>;

/// Represents part of a request to apply/change a score.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RatingRequest {
    score: Rating,
}

impl Deref for RatingRequest {
    type Target = Rating;
    fn deref(&self) -> &Self::Target {
        &self.score
    }
}

/// Represents part of a request to delete a resource.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeleteRequest {
    version: DateTime,
}

impl Deref for DeleteRequest {
    type Target = DateTime;
    fn deref(&self) -> &Self::Target {
        &self.version
    }
}

/// Represents part of a request to merge two resources.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MergeRequest<T> {
    remove: T,
    merge_to: T,
    remove_version: DateTime,
    merge_to_version: DateTime,
}

/// Represents part of a request to retrieve one or more resources.
#[derive(Deserialize)]
struct ResourceQuery {
    query: Option<String>,
    fields: Option<String>,
    #[serde(rename = "bump-login")]
    bump_login: Option<bool>,
}

impl ResourceQuery {
    fn criteria(&self) -> &str {
        self.query.as_deref().unwrap_or("")
    }

    fn fields(&self) -> Option<&str> {
        self.fields.as_deref()
    }

    fn bump_login(&self, client: Option<AuthUser>) -> ApiResult<()> {
        match (client, self.bump_login) {
            (Some(user), Some(true)) => update::user::last_login_time(user.id),
            _ => Ok(()),
        }
    }
}

/// Represents part of a request to retrieve multiple resources, paged.
#[derive(Deserialize)]
struct PagedQuery {
    offset: Option<i64>,
    limit: NonZero<i64>,
    #[serde(flatten)]
    query: ResourceQuery,
}

impl PagedQuery {
    fn criteria(&self) -> &str {
        self.query.criteria()
    }

    fn fields(&self) -> Option<&str> {
        self.query.fields()
    }

    fn bump_login(&self, user: Option<AuthUser>) -> ApiResult<()> {
        self.query.bump_login(user)
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

/// Represents a response if an error occured.
#[derive(Serialize)]
struct ErrorResponse {
    title: &'static str,
    name: &'static str,
    description: String,
}

/// Returns the rank of `client`.
fn client_access_level(client: Option<AuthUser>) -> UserRank {
    client.map(|user| user.rank).unwrap_or(UserRank::Anonymous)
}

/// Checks if `current_version` matches `client_version`.
/// Returns error if they do not match.
fn verify_version(current_version: DateTime, client_version: DateTime) -> ApiResult<()> {
    if cfg!(test) {
        Ok(())
    } else {
        (current_version == client_version)
            .then_some(())
            .ok_or(Error::ResourceModified)
    }
}

/// Optionally extracts an authorization header from the incoming request and attempts to authenticate with it.
fn auth() -> impl Filter<Extract = (AuthResult,), Error = Rejection> + Clone {
    warp::header::optional("authorization").map(|auth: Option<_>| auth.map(header::authenticate_user).transpose())
}

async fn empty_query(_err: Rejection) -> Result<(ResourceQuery,), Infallible> {
    Ok((ResourceQuery {
        query: None,
        fields: None,
        bump_login: None,
    },))
}

/// Optionally serializes a resource query.
fn resource_query() -> impl Filter<Extract = (ResourceQuery,), Error = Infallible> + Clone {
    warp::query::<ResourceQuery>().or_else(empty_query)
}

// Any value that is present is considered Some value, including null.
fn deserialize_some<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}
