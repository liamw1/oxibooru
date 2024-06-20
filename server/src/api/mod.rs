pub mod info;
pub mod pool_category;
pub mod tag_category;
pub mod user;

use serde::Serialize;
use thiserror::Error;
use warp::http::StatusCode;
use warp::reply::Json;
use warp::reply::Response;
use warp::reply::WithStatus;

pub enum Reply {
    Json(Json),
    WithStatus(WithStatus<String>),
}

impl Reply {
    fn from<T: Serialize>(result: Result<T, ApiError>) -> Self {
        match result {
            Ok(response) => Self::Json(warp::reply::json(&response)),
            Err(err) => {
                println!("ERROR: {err}");
                Self::WithStatus(warp::reply::with_status(err.to_string(), err.status_code()))
            }
        }
    }
}

impl warp::Reply for Reply {
    fn into_response(self) -> Response {
        match self {
            Self::Json(reply) => reply.into_response(),
            Self::WithStatus(reply) => reply.into_response(),
        }
    }
}

#[derive(Debug, Error)]
#[error(transparent)]
enum ApiError {
    FailedConnection(#[from] diesel::ConnectionError),
    FailedQuery(#[from] diesel::result::Error),
    #[error("Insufficient privileges")]
    InsufficientPrivileges,
    #[error("Missing '{0}' in header")]
    MissingBodyParam(&'static str),
    BadHeader(#[from] warp::http::header::ToStrError),
    BadBody(#[from] serde_json::Error),
    BadHash(#[from] crate::auth::HashError),
}

impl ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::FailedConnection(_) => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::FailedQuery(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::InsufficientPrivileges => StatusCode::FORBIDDEN,
            ApiError::MissingBodyParam(_) => StatusCode::BAD_REQUEST,
            ApiError::BadHeader(_) => StatusCode::BAD_REQUEST,
            ApiError::BadBody(_) => StatusCode::BAD_REQUEST,
            ApiError::BadHash(_) => StatusCode::BAD_REQUEST,
        }
    }
}

/*
    We read in request body as bytes because the client doesn't utf-8 encode them,
    so warp::body::json can't be used. Instead, we interpret incoming bytes as chars,
    encode them into a String, and then parse as a json. TODO: Fix client
*/
fn parse_body(body: warp::hyper::body::Bytes) -> Result<serde_json::Value, ApiError> {
    let utf8_body = body.into_iter().map(|b| b as char).collect::<String>();
    serde_json::from_str(&utf8_body).map_err(ApiError::from)
}
