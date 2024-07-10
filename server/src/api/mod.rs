pub mod comment;
pub mod info;
pub mod micro;
pub mod pool_category;
pub mod post;
pub mod tag;
pub mod tag_category;
pub mod upload;
pub mod user;
pub mod user_token;

use crate::auth::header::{self, AuthenticationError};
use crate::error::ErrorKind;
use crate::model::enums::UserRank;
use crate::model::user::User;
use crate::util::DateTime;
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
    #[error("Resource is out-of-date")]
    OutOfDate,
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
            Self::OutOfDate => StatusCode::CONFLICT,
            Self::ResourceDoesNotExist => StatusCode::GONE,
            Self::ResourceModified => StatusCode::CONFLICT,
            Self::WarpError(_) => StatusCode::BAD_REQUEST,
        }
    }

    fn category(&self) -> &'static str {
        match self {
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
            Self::OutOfDate => "Out-of-date",
            Self::ResourceDoesNotExist => "Resource Does Not Exist",
            Self::ResourceModified => "Resource Modified",
            Self::WarpError(_) => "Warp Error",
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

type AuthResult = Result<Option<User>, AuthenticationError>;

#[derive(Deserialize)]
struct ResourceVersion {
    version: DateTime,
}

#[derive(Deserialize)]
struct PagedQuery {
    offset: Option<i64>,
    limit: i64,
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
    title: &'static str,
    name: &'static str,
    description: String,
}

fn client_access_level(client: Option<&User>) -> UserRank {
    client.map(|user| user.rank).unwrap_or(UserRank::Anonymous)
}

fn verify_privilege(client: Option<&User>, required_rank: UserRank) -> Result<(), Error> {
    (client_access_level(client) >= required_rank)
        .then_some(())
        .ok_or(Error::InsufficientPrivileges)
}

fn verify_version(current_version: DateTime, client_version: ResourceVersion) -> Result<(), Error> {
    (current_version == client_version.version)
        .then_some(())
        .ok_or(Error::OutOfDate)
}

fn auth() -> impl Filter<Extract = (AuthResult,), Error = Rejection> + Clone {
    warp::header::optional("authorization")
        .map(|opt_auth: Option<_>| opt_auth.map(|auth| header::authenticate_user(auth)).transpose())
}
