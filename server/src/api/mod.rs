pub mod comment;
pub mod info;
pub mod pool;
pub mod pool_category;
pub mod post;
pub mod tag;
pub mod tag_category;
pub mod upload;
pub mod user;
pub mod user_token;

use crate::auth::header::{self, AuthenticationError};
use crate::config::{self, RegexType};
use crate::error::ErrorKind;
use crate::model::enums::{Rating, UserRank};
use crate::model::user::User;
use crate::util::DateTime;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::num::NonZero;
use std::ops::Deref;
use warp::http::StatusCode;
use warp::reply::Json;
use warp::reply::Response;
use warp::reply::WithStatus;
use warp::Filter;
use warp::Rejection;

// TODO: Bump-login

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
    #[error("Multi-part form error")]
    BadMultiPartForm,
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
    FailedQuery(#[from] diesel::result::Error),
    #[error("Upload failed")]
    FailedUpload,
    FromStr(#[from] Box<dyn std::error::Error>),
    #[error("Insufficient privileges")]
    InsufficientPrivileges,
    Image(#[from] image::ImageError),
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
    VideoDecoding(#[from] crate::content::decode::VideoDecodingError),
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
            Self::BadMultiPartForm => StatusCode::BAD_REQUEST,
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
            Self::FailedQuery(err) => query_error_status_code(err),
            Self::FailedUpload => StatusCode::INTERNAL_SERVER_ERROR,
            Self::FromStr(_) => StatusCode::BAD_REQUEST,
            Self::InsufficientPrivileges => StatusCode::FORBIDDEN,
            Self::Image(_) => StatusCode::INTERNAL_SERVER_ERROR,
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
            Self::BadMultiPartForm => "Bad Multi-Part Form",
            Self::ContentTypeMismatch => "Content Type Mismatch",
            Self::CyclicDependency => "Cyclic Dependency",
            Self::DeleteDefault => "Delete Default",
            Self::ExpressionFailsRegex => "Expression Fails Regex",
            Self::FailedAuthentication(_) => "Failed Authentication",
            Self::FailedConnection(_) => "Failed Connection",
            Self::FailedQuery(_) => "Failed Query",
            Self::FailedUpload => "Failed Upload",
            Self::FromStr(_) => "FromStr Error",
            Self::InsufficientPrivileges => "Insufficient Privileges",
            Self::Image(_) => "Image Error",
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

pub fn routes() -> impl Filter<Extract = impl warp::Reply, Error = Infallible> + Clone {
    let catch_all = warp::any().map(|| {
        eprintln!("Unimplemented request!");
        warp::reply::with_status("Bad Request", StatusCode::BAD_REQUEST)
    });
    let log = warp::filters::log::custom(|info| {
        println!("{} {} [{}]", info.method(), info.path(), info.status());
    });

    info::routes()
        .or(comment::routes())
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
}

impl ResourceQuery {
    fn criteria(&self) -> &str {
        self.query.as_deref().unwrap_or("")
    }

    fn fields(&self) -> Option<&str> {
        self.fields.as_deref()
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
    },))
}

/*
    Optionally serializes a resource query
*/
fn resource_query() -> impl Filter<Extract = (ResourceQuery,), Error = Infallible> + Clone {
    warp::query::<ResourceQuery>().or_else(empty_query)
}
