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
use crate::error::ErrorKind;
use crate::model::enums::UserRank;
use crate::model::user::User;
use crate::util::DateTime;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::ops::Deref;
use warp::http::StatusCode;
use warp::reply::Json;
use warp::reply::Response;
use warp::reply::WithStatus;
use warp::Filter;
use warp::Rejection;

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
    #[error("Multi-part form error")]
    BadMultiPartForm,
    #[error("Request content-type did not match file extension")]
    ContentTypeMismatch,
    FailedAuthentication(#[from] AuthenticationError),
    FailedConnection(#[from] diesel::ConnectionError),
    FailedQuery(#[from] diesel::result::Error),
    FromStrError(#[from] Box<dyn std::error::Error>),
    #[error("Insufficient privileges")]
    InsufficientPrivileges,
    ImageError(#[from] image::ImageError),
    IoError(#[from] std::io::Error),
    NotAnInteger(#[from] std::num::ParseIntError),
    #[error("Someone else modified this in the meantime. Please try again.")]
    ResourceModified,
    SearchError(#[from] crate::search::Error),
    Utf8Conversion(#[from] std::str::Utf8Error),
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
            Self::BadMultiPartForm => StatusCode::BAD_REQUEST,
            Self::ContentTypeMismatch => StatusCode::BAD_REQUEST,
            Self::FailedAuthentication(err) => match err {
                AuthenticationError::FailedConnection(_) => StatusCode::SERVICE_UNAVAILABLE,
                AuthenticationError::FailedQuery(err) => query_error_status_code(err),
                _ => StatusCode::UNAUTHORIZED,
            },
            Self::FailedConnection(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::FailedQuery(err) => query_error_status_code(err),
            Self::FromStrError(_) => StatusCode::BAD_REQUEST,
            Self::InsufficientPrivileges => StatusCode::FORBIDDEN,
            Self::ImageError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::IoError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::NotAnInteger(_) => StatusCode::BAD_REQUEST,
            Self::ResourceModified => StatusCode::CONFLICT,
            Self::SearchError(_) => StatusCode::BAD_REQUEST,
            Self::Utf8Conversion(_) => StatusCode::BAD_REQUEST,
            Self::WarpError(_) => StatusCode::BAD_REQUEST,
        }
    }

    fn category(&self) -> &'static str {
        match self {
            Self::BadExtension(_) => "Bad Extension",
            Self::BadHash(_) => "Bad Hash",
            Self::BadHeader(_) => "Bad Header",
            Self::BadMultiPartForm => "Bad Multi-Part Form",
            Self::ContentTypeMismatch => "Content Type Mismatch",
            Self::FailedAuthentication(_) => "Failed Authentication",
            Self::FailedConnection(_) => "Failed Connection",
            Self::FailedQuery(_) => "Failed Query",
            Self::FromStrError(_) => "FromStr Error",
            Self::InsufficientPrivileges => "Insufficient Privileges",
            Self::ImageError(_) => "Image Error",
            Self::IoError(_) => "IO Error",
            Self::NotAnInteger(_) => "Parse Int Error",
            Self::ResourceModified => "Resource Modified",
            Self::SearchError(_) => "Search Error",
            Self::Utf8Conversion(_) => "Utf8 Conversion Error",
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

pub fn verify_privilege(client: Option<&User>, required_rank: UserRank) -> Result<(), Error> {
    (client_access_level(client) >= required_rank)
        .then_some(())
        .ok_or(Error::InsufficientPrivileges)
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
        .or(comment::routes())
        .or(pool_category::routes())
        .or(pool::routes())
        .or(post::routes())
        .or(tag_category::routes())
        .or(tag::routes())
        .or(upload::routes())
        .or(user_token::routes())
        .or(user::routes())
        .or(warp::any()
            .and(warp::body::bytes())
            .map(|bytes: warp::hyper::body::Bytes| {
                println!("Request body: {}", std::str::from_utf8(&bytes).expect("error converting bytes to &str"));
                warp::reply::with_status("Bad Request", StatusCode::BAD_REQUEST)
            }))
        .or(catch_all)
        .with(log)
}

type AuthResult = Result<Option<User>, AuthenticationError>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceVersion {
    version: DateTime,
}

impl Deref for ResourceVersion {
    type Target = DateTime;
    fn deref(&self) -> &Self::Target {
        &self.version
    }
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
    limit: i64,
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
struct PagedResponse<T: Serialize> {
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

fn verify_version(current_version: DateTime, client_version: DateTime) -> Result<(), Error> {
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
