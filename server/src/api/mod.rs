use crate::auth::Client;
use crate::auth::header::{self, AuthenticationError};
use crate::config::RegexType;
use crate::error::ErrorKind;
use crate::model::enums::{MimeType, Rating, ResourceType, UserRank};
use crate::string::SmallString;
use crate::time::DateTime;
use crate::{config, update};
use axum::extract::Request;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use serde::{Deserialize, Deserializer, Serialize};
use std::num::NonZero;
use std::ops::Deref;
use std::time::Duration;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

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

pub type ApiResult<T> = Result<T, Error>;

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub enum Error {
    BadExtension(#[from] crate::model::enums::ParseExtensionError),
    BadHash(#[from] crate::auth::HashError),
    BadHeader(#[from] axum::http::header::ToStrError),
    #[error("File of type {0} did not match request with content-type '{1}'")]
    ContentTypeMismatch(MimeType, SmallString),
    #[error("Cyclic dependency detected in {0}s")]
    CyclicDependency(ResourceType),
    #[error("Cannot delete default {0}")]
    DeleteDefault(ResourceType),
    #[error("SWF has no decodable images")]
    EmptySwf,
    #[error("Video file has no frames")]
    EmptyVideo,
    #[error("'{0}' does not match on {1} regex")]
    ExpressionFailsRegex(SmallString, RegexType),
    FailedAuthentication(#[from] AuthenticationError),
    FailedConnection(#[from] diesel::r2d2::PoolError),
    FailedEmailTransport(#[from] lettre::transport::smtp::Error),
    FailedQuery(#[from] diesel::result::Error),
    FromStr(#[from] Box<dyn std::error::Error + Send + Sync>),
    #[error("Insufficient privileges")]
    InsufficientPrivileges,
    InvalidEmailAddress(#[from] lettre::address::AddressError),
    InvalidEmail(#[from] lettre::error::Error),
    InvalidHeader(#[from] reqwest::header::InvalidHeaderValue),
    #[error("Metadata must be application/json")]
    InvalidMetadataType,
    #[error("Invalid sort token")]
    InvalidSort,
    InvalidTime(#[from] crate::search::TimeParsingError),
    #[error("Cannot create an anonymous user")]
    InvalidUserRank,
    Image(#[from] image::ImageError),
    JsonSerialization(#[from] serde_json::Error),
    #[error("Missing {0} content")]
    MissingContent(ResourceType),
    #[error("Form is missing content-type")]
    MissingContentType,
    #[error("Missing form data")]
    MissingFormData,
    #[error("Missing metadata form")]
    MissingMetadata,
    #[error("Missing smtp info")]
    MissingSmtpInfo,
    Multipart(#[from] axum::extract::multipart::MultipartError),
    #[error("User has no email")]
    NoEmail,
    #[error("{0} needs at least one name")]
    NoNamesGiven(ResourceType),
    NotAnInteger(#[from] std::num::ParseIntError),
    #[error("{0} not found")]
    NotFound(ResourceType),
    #[error("This action requires you to be logged in")]
    NotLoggedIn,
    Request(#[from] reqwest::Error),
    #[error("Someone else modified this in the meantime. Please try again.")]
    ResourceModified,
    #[error("Cannot merge {0} with itself")]
    SelfMerge(ResourceType),
    StdIo(#[from] std::io::Error),
    SwfDecoding(#[from] swf::error::Error),
    #[error("Password reset token is invalid")]
    UnauthorizedPasswordReset,
    Utf8Conversion(#[from] std::str::Utf8Error),
    VideoDecoding(#[from] video_rs::Error),
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
            Self::EmptySwf => StatusCode::BAD_REQUEST,
            Self::EmptyVideo => StatusCode::BAD_REQUEST,
            Self::ExpressionFailsRegex(..) => StatusCode::BAD_GATEWAY,
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
            Self::InvalidHeader(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::InvalidMetadataType => StatusCode::BAD_REQUEST,
            Self::InvalidSort => StatusCode::BAD_REQUEST,
            Self::InvalidTime(_) => StatusCode::BAD_REQUEST,
            Self::InvalidUserRank => StatusCode::BAD_REQUEST,
            Self::Image(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::JsonSerialization(err) => match err.classify() {
                Category::Io | Category::Eof => StatusCode::INTERNAL_SERVER_ERROR,
                Category::Syntax | Category::Data => StatusCode::BAD_REQUEST,
            },
            Self::MissingContent(_) => StatusCode::BAD_REQUEST,
            Self::MissingContentType => StatusCode::BAD_REQUEST,
            Self::MissingFormData => StatusCode::BAD_REQUEST,
            Self::MissingMetadata => StatusCode::BAD_REQUEST,
            Self::MissingSmtpInfo => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Multipart(_) => StatusCode::BAD_REQUEST,
            Self::NoEmail => StatusCode::BAD_REQUEST,
            Self::NoNamesGiven(_) => StatusCode::BAD_REQUEST,
            Self::NotAnInteger(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::NotLoggedIn => StatusCode::FORBIDDEN,
            Self::Request(_) => StatusCode::BAD_REQUEST,
            Self::ResourceModified => StatusCode::CONFLICT,
            Self::SelfMerge(_) => StatusCode::BAD_REQUEST,
            Self::StdIo(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::SwfDecoding(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::UnauthorizedPasswordReset => StatusCode::UNAUTHORIZED,
            Self::Utf8Conversion(_) => StatusCode::BAD_REQUEST,
            Self::VideoDecoding(_) => StatusCode::BAD_REQUEST,
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
            Self::EmptySwf => "Empty SWF",
            Self::EmptyVideo => "Empty Video",
            Self::ExpressionFailsRegex(..) => "Expression Fails Regex",
            Self::FailedAuthentication(_) => "Failed Authentication",
            Self::FailedConnection(_) => "Failed Connection",
            Self::FailedEmailTransport(_) => "Failed Email Transport",
            Self::FailedQuery(_) => "Failed Query",
            Self::FromStr(_) => "FromStr Error",
            Self::InsufficientPrivileges => "Insufficient Privileges",
            Self::InvalidEmailAddress(_) => "Invalid Email Address",
            Self::InvalidEmail(_) => "Invalid Email",
            Self::InvalidHeader(_) => "Invalid Header",
            Self::InvalidMetadataType => "Invalid Metadata Type",
            Self::InvalidSort => "Invalid Sort",
            Self::InvalidTime(_) => "Invalid Time",
            Self::InvalidUserRank => "Invalid User Rank",
            Self::Image(_) => "Image Error",
            Self::JsonSerialization(_) => "JSON Serialization Error",
            Self::MissingContent(_) => "Missing Content",
            Self::MissingContentType => "Missing Content Type",
            Self::MissingFormData => "Missing Form Data",
            Self::MissingMetadata => "Missing Metadata",
            Self::MissingSmtpInfo => "Missing SMTP Info",
            Self::Multipart(_) => "Multipart/Form-Data Error",
            Self::NoEmail => "No Email",
            Self::NoNamesGiven(_) => "No Names Given",
            Self::NotAnInteger(_) => "Parse Int Error",
            Self::NotFound(_) => "Resource Not Found",
            Self::NotLoggedIn => "Not Logged In",
            Self::Request(_) => "Request Error",
            Self::ResourceModified => "Resource Modified",
            Self::SelfMerge(_) => "Self Merge",
            Self::StdIo(_) => "IO Error",
            Self::SwfDecoding(_) => "SWF Decoding Error",
            Self::UnauthorizedPasswordReset => "Unauthorized Password Reset",
            Self::Utf8Conversion(_) => "Utf8 Conversion Error",
            Self::VideoDecoding(_) => "Video Decoding Error",
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

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (self.status_code(), Json(self.response())).into_response()
    }
}

/// Checks if the `client` is at least `required_rank`.
/// Returns error if client is lower rank than `required_rank`.
pub fn verify_privilege(client: Client, required_rank: UserRank) -> ApiResult<()> {
    (client.rank >= required_rank)
        .then_some(())
        .ok_or(Error::InsufficientPrivileges)
}

/// Checks if `haystack` matches regex `regex_type`.
/// Returns error if it does not match on the regex.
pub fn verify_matches_regex(haystack: &str, regex_type: RegexType) -> ApiResult<()> {
    config::regex(regex_type)
        .is_match(haystack)
        .then_some(())
        .ok_or_else(|| Error::ExpressionFailsRegex(SmallString::new(haystack), regex_type))
}

/// Checks if `email` is a valid email.
/// Returns error if `email` is invalid.
pub fn verify_valid_email(email: Option<&str>) -> Result<(), lettre::address::AddressError> {
    match email {
        Some(address) => address.parse::<lettre::Address>().map(|_| ()),
        None => Ok(()),
    }
}

pub fn routes() -> Router {
    Router::new()
        .merge(comment::routes())
        .merge(info::routes())
        .merge(password_reset::routes())
        .merge(pool_category::routes())
        .merge(pool::routes())
        .merge(post::routes())
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
        .route_layer(middleware::from_fn(auth))
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

/// Represents a response if an error occured.
#[derive(Serialize)]
struct ErrorResponse {
    title: &'static str,
    name: &'static str,
    description: String,
}

/// Checks if `current_version` matches `client_version`.
/// Returns error if they do not match.
fn verify_version(current_version: DateTime, client_version: DateTime) -> ApiResult<()> {
    (cfg!(test) || current_version == client_version)
        .then_some(())
        .ok_or(Error::ResourceModified)
}

async fn auth(mut request: Request, next: Next) -> ApiResult<Response> {
    let auth_header = request.headers().get(AUTHORIZATION);
    let client = if let Some(auth_value) = auth_header {
        let auth_str = auth_value.to_str()?;
        header::authenticate_user(auth_str)
    } else {
        Ok(Client::new(None, UserRank::Anonymous))
    }?;

    // If client is not anonymous and query contains "bump-login", update login time
    if let Some(user_id) = client.id
        && let Some(query) = request.uri().query()
        && query.contains("bump-login")
    {
        update::user::last_login_time(user_id)?;
    }

    request.extensions_mut().insert(client);
    Ok(next.run(request).await)
}

// Any value that is present is considered Some value, including null.
fn deserialize_some<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}
