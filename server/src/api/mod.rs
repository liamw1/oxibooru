pub mod info;
pub mod pool_category;
pub mod tag_category;

use thiserror::Error;
use warp::http::StatusCode;
use warp::reply;
use warp::reply::WithStatus;

#[derive(Debug, Error)]
#[error(transparent)]
enum ApiError {
    FailedConnection(#[from] diesel::ConnectionError),
    FailedQuery(#[from] diesel::result::Error),
    #[error("Insufficient privileges")]
    InsufficientPrivileges,
}

impl ApiError {
    fn to_reply(self) -> WithStatus<String> {
        match self {
            ApiError::FailedConnection(err) => reply::with_status(err.to_string(), StatusCode::SERVICE_UNAVAILABLE),
            ApiError::FailedQuery(err) => reply::with_status(err.to_string(), StatusCode::INTERNAL_SERVER_ERROR),
            ApiError::InsufficientPrivileges => {
                reply::with_status("Insufficient privileges".into(), StatusCode::FORBIDDEN)
            }
        }
    }
}
