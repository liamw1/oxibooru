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

use crate::auth::header::{self, AuthenticationError};
use crate::config::{self, RegexType};
use crate::error::ErrorKind;
use crate::model::enums::{Rating, UserRank};
use crate::model::user::User;
use crate::time::DateTime;
use crate::update;
use serde::{Deserialize, Serialize};
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
    #[error("Request content-type did not match file extension")]
    ContentTypeMismatch,
    #[error("Cyclic dependency detected")]
    CyclicDependency,
    #[error("Cannot delete default category")]
    DeleteDefault,
    #[error("Expression does not match on regex")]
    ExpressionFailsRegex,
    FailedAuthentication(#[from] AuthenticationError),
    FailedConnection(#[from] diesel::r2d2::PoolError),
    #[error("Failed to send email. Reason: {0}")]
    FailedEmailTransport(String),
    FailedQuery(#[from] diesel::result::Error),
    #[error("Upload failed")]
    FailedUpload,
    FromStr(#[from] Box<dyn std::error::Error>),
    #[error("Insufficient privileges")]
    InsufficientPrivileges,
    InvalidEmailAddress(#[from] lettre::address::AddressError),
    InvalidEmail(#[from] lettre::error::Error),
    Image(#[from] image::ImageError),
    #[error("Missing form data")]
    MissingFormData,
    #[error("Missing smtp info")]
    MissingSmtpInfo,
    #[error("User has no email")]
    NoEmail,
    #[error("Resource needs at least one name")]
    NoNamesGiven,
    NotAnInteger(#[from] std::num::ParseIntError),
    #[error("This action requires you to be logged in")]
    NotLoggedIn,
    #[error("Someone else modified this in the meantime. Please try again.")]
    ResourceModified,
    Search(#[from] crate::search::Error),
    #[error("Cannot merge resource with itself")]
    SelfMerge,
    StdIo(#[from] std::io::Error),
    Utf8Conversion(#[from] std::str::Utf8Error),
    VideoDecoding(#[from] video_rs::Error),
    Warp(#[from] warp::Error),
}

impl Error {
    fn status_code(&self) -> StatusCode {
        type QueryError = diesel::result::Error;

        let query_error_status_code = |err: &QueryError| match err {
            QueryError::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        match self {
            Self::BadExtension(_) => StatusCode::BAD_REQUEST,
            Self::BadHash(_) => StatusCode::BAD_REQUEST,
            Self::BadHeader(_) => StatusCode::BAD_REQUEST,
            Self::ContentTypeMismatch => StatusCode::BAD_REQUEST,
            Self::CyclicDependency => StatusCode::BAD_REQUEST,
            Self::DeleteDefault => StatusCode::BAD_REQUEST,
            Self::ExpressionFailsRegex => StatusCode::BAD_GATEWAY,
            Self::FailedAuthentication(err) => match err {
                AuthenticationError::FailedConnection(_) => StatusCode::SERVICE_UNAVAILABLE,
                AuthenticationError::FailedQuery(err) => query_error_status_code(err),
                _ => StatusCode::UNAUTHORIZED,
            },
            Self::FailedConnection(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::FailedEmailTransport(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::FailedQuery(err) => query_error_status_code(err),
            Self::FailedUpload => StatusCode::INTERNAL_SERVER_ERROR,
            Self::FromStr(_) => StatusCode::BAD_REQUEST,
            Self::InsufficientPrivileges => StatusCode::FORBIDDEN,
            Self::InvalidEmailAddress(_) => StatusCode::BAD_REQUEST,
            Self::InvalidEmail(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Image(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::MissingFormData => StatusCode::BAD_REQUEST,
            Self::MissingSmtpInfo => StatusCode::INTERNAL_SERVER_ERROR,
            Self::NoEmail => StatusCode::BAD_REQUEST,
            Self::NoNamesGiven => StatusCode::BAD_REQUEST,
            Self::NotAnInteger(_) => StatusCode::BAD_REQUEST,
            Self::NotLoggedIn => StatusCode::FORBIDDEN,
            Self::ResourceModified => StatusCode::CONFLICT,
            Self::Search(_) => StatusCode::BAD_REQUEST,
            Self::SelfMerge => StatusCode::BAD_REQUEST,
            Self::StdIo(_) => StatusCode::INTERNAL_SERVER_ERROR,
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
            Self::ContentTypeMismatch => "Content Type Mismatch",
            Self::CyclicDependency => "Cyclic Dependency",
            Self::DeleteDefault => "Delete Default",
            Self::ExpressionFailsRegex => "Expression Fails Regex",
            Self::FailedAuthentication(_) => "Failed Authentication",
            Self::FailedConnection(_) => "Failed Connection",
            Self::FailedEmailTransport(_) => "Failed Email Transport",
            Self::FailedQuery(_) => "Failed Query",
            Self::FailedUpload => "Failed Upload",
            Self::FromStr(_) => "FromStr Error",
            Self::InsufficientPrivileges => "Insufficient Privileges",
            Self::InvalidEmailAddress(_) => "Invalid Email Address",
            Self::InvalidEmail(_) => "Invalid Email",
            Self::Image(_) => "Image Error",
            Self::MissingFormData => "Missing Form Data",
            Self::MissingSmtpInfo => "Missing SMTP Info",
            Self::NoEmail => "No Email",
            Self::NoNamesGiven => "No Names Given",
            Self::NotAnInteger(_) => "Parse Int Error",
            Self::NotLoggedIn => "Not Logged In",
            Self::ResourceModified => "Resource Modified",
            Self::Search(_) => "Search Error",
            Self::SelfMerge => "Self Merge",
            Self::StdIo(_) => "IO Error",
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

pub fn verify_privilege(client: Option<&User>, required_rank: UserRank) -> ApiResult<()> {
    (client_access_level(client) >= required_rank)
        .then_some(())
        .ok_or(Error::InsufficientPrivileges)
}

pub fn verify_matches_regex(haystack: &str, regex_type: RegexType) -> ApiResult<()> {
    config::regex(regex_type)
        .is_match(haystack)
        .then_some(())
        .ok_or(Error::ExpressionFailsRegex)
}

pub fn verify_valid_email(email: Option<&str>) -> Result<(), lettre::address::AddressError> {
    match email {
        Some(address) => address.parse::<lettre::Address>().map(|_| ()),
        None => Ok(()),
    }
}

pub fn routes() -> impl Filter<Extract = impl warp::Reply, Error = Infallible> + Clone {
    // let catch_body = warp::put()
    //     .and(warp::body::bytes())
    //     .map(|body: warp::hyper::body::Bytes| {
    //         eprintln!("Bad request with body");
    //         if let Ok(body_string) = std::str::from_utf8(&body) {
    //             eprintln!("{body_string}");
    //         }
    //         warp::reply::with_status("Bad Request", StatusCode::BAD_REQUEST)
    //     });
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

type AuthResult = Result<Option<User>, AuthenticationError>;

const MAX_UPLOAD_SIZE: u64 = 4 * 1024 * 1024 * 1024;

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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MergeRequest<T> {
    remove: T,
    merge_to: T,
    remove_version: DateTime,
    merge_to_version: DateTime,
}

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

    fn bump_login(&self, user: Option<&User>) -> ApiResult<()> {
        match (user, self.bump_login) {
            (Some(user), Some(true)) => update::last_login_time(user),
            _ => Ok(()),
        }
    }
}

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

    fn bump_login(&self, user: Option<&User>) -> ApiResult<()> {
        self.query.bump_login(user)
    }
}

#[derive(Serialize)]
struct UnpagedResponse<T> {
    results: Vec<T>,
}

#[derive(Serialize)]
struct PagedResponse<T> {
    query: Option<String>,
    offset: i64,
    limit: i64,
    total: i64,
    results: Vec<T>,
}

#[derive(Serialize)]
struct ErrorResponse {
    title: &'static str,
    name: &'static str,
    description: String,
}

fn client_access_level(client: Option<&User>) -> UserRank {
    client.map(|user| user.rank).unwrap_or(UserRank::Anonymous)
}

fn verify_version(current_version: DateTime, client_version: DateTime) -> ApiResult<()> {
    (current_version == client_version)
        .then_some(())
        .ok_or(Error::ResourceModified)
}

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

/*
    Optionally serializes a resource query
*/
fn resource_query() -> impl Filter<Extract = (ResourceQuery,), Error = Infallible> + Clone {
    warp::query::<ResourceQuery>().or_else(empty_query)
}
