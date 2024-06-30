pub mod comment;
pub mod info;
pub mod micro;
pub mod pool_category;
pub mod post;
pub mod tag_category;
pub mod upload;
pub mod user;
pub mod user_token;

use crate::auth::header::{self, AuthenticationError};
use crate::error::ErrorKind;
use crate::model::enums::UserRank;
use crate::model::user::User;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use warp::http::StatusCode;
use warp::reply::Json;
use warp::reply::Response;
use warp::reply::WithStatus;
use warp::Filter;
use warp::Rejection;

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

impl<T: Serialize> From<Result<T, Error>> for Reply {
    fn from(value: Result<T, Error>) -> Self {
        match value {
            Ok(response) => Self::Json(warp::reply::json(&response)),
            Err(err) => {
                println!("ERROR: {err}");
                let response = warp::reply::json(&err.response());
                Self::WithStatus(warp::reply::with_status(response, err.status_code()))
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub enum Error {
    BadBody(#[from] serde_json::Error),
    BadExtension(#[from] crate::model::enums::ParseExtensionError),
    BadHash(#[from] crate::auth::HashError),
    BadHeader(#[from] warp::http::header::ToStrError),
    BadMimeType(#[from] crate::model::enums::ParseMimeTypeError),
    #[error("Multi-part form error")]
    BadMultiPartForm,
    BadUserPrivilege(#[from] crate::model::enums::ParseUserRankError),
    #[error("Request content-type did not match file extension")]
    ContentTypeMismatch,
    FailedAuthentication(#[from] AuthenticationError),
    FailedConnection(#[from] diesel::ConnectionError),
    FailedQuery(#[from] diesel::result::Error),
    #[error("Insufficient privileges")]
    InsufficientPrivileges,
    ImageError(#[from] image::ImageError),
    IoError(#[from] std::io::Error),
    #[error("Resource does not exist")]
    ResourceDoesNotExist,
    // Someone else modified this in the meantime. Please try again.
    #[error("Resouce was modified by someone else")]
    ResourceModified,
    WarpError(#[from] warp::Error),
}

impl Error {
    fn status_code(&self) -> StatusCode {
        type QueryError = diesel::result::Error;

        let query_error_status_code = |err: &QueryError| match err {
            QueryError::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        match self {
            Self::BadBody(_) => StatusCode::BAD_REQUEST,
            Self::BadExtension(_) => StatusCode::BAD_REQUEST,
            Self::BadHash(_) => StatusCode::BAD_REQUEST,
            Self::BadHeader(_) => StatusCode::BAD_REQUEST,
            Self::BadMimeType(_) => StatusCode::BAD_REQUEST,
            Self::BadMultiPartForm => StatusCode::BAD_REQUEST,
            Self::BadUserPrivilege(_) => StatusCode::BAD_REQUEST,
            Self::ContentTypeMismatch => StatusCode::BAD_REQUEST,
            Self::FailedAuthentication(err) => match err {
                AuthenticationError::FailedConnection(_) => StatusCode::SERVICE_UNAVAILABLE,
                AuthenticationError::FailedQuery(err) => query_error_status_code(err),
                _ => StatusCode::UNAUTHORIZED,
            },
            Self::FailedConnection(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::FailedQuery(err) => query_error_status_code(err),
            Self::InsufficientPrivileges => StatusCode::FORBIDDEN,
            Self::ImageError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::IoError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::ResourceDoesNotExist => StatusCode::GONE,
            Self::ResourceModified => StatusCode::CONFLICT,
            Self::WarpError(_) => StatusCode::BAD_REQUEST,
        }
    }

    fn category(&self) -> &'static str {
        match self {
            Self::BadBody(_) => "Bad Body",
            Self::BadExtension(_) => "Bad Extension",
            Self::BadHash(_) => "Bad Hash",
            Self::BadHeader(_) => "Bad Header",
            Self::BadMimeType(_) => "Bad MIME Type",
            Self::BadMultiPartForm => "Bad Multi-Part Form",
            Self::BadUserPrivilege(_) => "Bad User Privilege",
            Self::ContentTypeMismatch => "Content Type Mismatch",
            Self::FailedAuthentication(_) => "Failed Authentication",
            Self::FailedConnection(_) => "Failed Connection",
            Self::FailedQuery(_) => "Failed Query",
            Self::InsufficientPrivileges => "Insufficient Privileges",
            Self::ImageError(_) => "Image Error",
            Self::IoError(_) => "IO Error",
            Self::ResourceDoesNotExist => "Resource Does Not Exist",
            Self::ResourceModified => "Resource Modified",
            Self::WarpError(_) => "Warp Error",
        }
    }

    fn response(&self) -> ErrorResponse {
        ErrorResponse {
            name: self.kind().to_owned(),
            title: self.category().to_owned(),
            description: self.to_string(),
        }
    }
}

impl ErrorKind for Error {
    fn kind(&self) -> &'static str {
        match self {
            Self::BadBody(err) => err.kind(),
            Self::BadExtension(_) => "BadExtension",
            Self::BadHash(err) => err.kind(),
            Self::BadHeader(_) => "BadHeader",
            Self::BadMimeType(_) => "BadMimeType",
            Self::BadMultiPartForm => "BadMultiPartForm",
            Self::BadUserPrivilege(_) => "BadUserPrivilege",
            Self::ContentTypeMismatch => "ContentTypeMismatch",
            Self::FailedAuthentication(err) => err.kind(),
            Self::FailedConnection(err) => err.kind(),
            Self::FailedQuery(err) => err.kind(),
            Self::InsufficientPrivileges => "InsufficientPrivileges",
            Self::ImageError(err) => err.kind(),
            Self::IoError(_) => "IOError",
            Self::ResourceDoesNotExist => "ResourceDoesNotExist",
            Self::ResourceModified => "ResourceModified",
            Self::WarpError(_) => "WarpError",
        }
    }
}

pub fn routes() -> impl Filter<Extract = impl warp::Reply, Error = Infallible> + Clone {
    let catch_all = warp::any().map(|| {
        println!("Unimplemented request!");
        warp::reply::with_status("Bad Request", StatusCode::BAD_REQUEST)
    });
    let log = warp::filters::log::custom(|info| {
        // println!("Header: {:?}", info.request_headers());
        println!("{} {} [{}]", info.method(), info.path(), info.status());
    });

    info::routes()
        .or(pool_category::routes())
        .or(post::routes())
        .or(tag_category::routes())
        .or(upload::routes())
        .or(user_token::routes())
        .or(user::routes())
        .or(catch_all)
        .with(log)
}

type AuthenticationResult = Result<Option<User>, Error>;

#[derive(Deserialize)]
struct PagedQuery {
    offset: Option<i64>,
    limit: Option<i64>,
    query: Option<String>,
}

#[derive(Serialize)]
struct PagedResponse<T: Serialize> {
    query: String,
    offset: i64,
    limit: i64,
    total: i64,
    results: Vec<T>,
}

#[derive(Serialize)]
struct ErrorResponse {
    title: String,
    name: String,
    description: String,
}

fn client_access_level(client: Option<&User>) -> UserRank {
    client.map(|user| user.rank).unwrap_or(UserRank::Anonymous)
}

fn access_level(auth_result: AuthenticationResult) -> Result<UserRank, Error> {
    auth_result.map(|client| client_access_level(client.as_ref()))
}

fn verify_privilege(client_rank: UserRank, requested_action: &str) -> Result<(), Error> {
    if !client_rank.has_permission_to(requested_action) {
        return Err(Error::InsufficientPrivileges);
    }
    Ok(())
}

fn log_body(body: &[u8]) {
    if !body.is_empty() {
        println!("Incoming body: {}", std::str::from_utf8(body).unwrap_or("ERROR: Failed to parse"));
    }
}

/*
    For some reason warp::body::json rejects incoming requests, perhaps due to encoding
    issues. Instead, we will parse the raw bytes into a deserialize-capable structure.
*/
fn parse_json_body<'a, T: serde::Deserialize<'a>>(body: &'a [u8]) -> Result<T, Error> {
    if body.is_empty() {
        serde_json::from_slice("{}".as_bytes()).map_err(Error::from)
    } else {
        log_body(body);
        serde_json::from_slice(body).map_err(Error::from)
    }
}

fn auth() -> impl Filter<Extract = (Result<Option<User>, Error>,), Error = Rejection> + Clone {
    warp::header::optional("authorization").map(|opt_auth: Option<_>| {
        opt_auth
            .map(|auth| header::authenticate_user(auth))
            .transpose()
            .map_err(Error::from)
    })
}
