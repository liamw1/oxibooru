pub mod info;
pub mod pool_category;
pub mod tag_category;
pub mod user;

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
        reply::with_status(self.to_string(), self.status_code())
    }

    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::FailedConnection(_) => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::FailedQuery(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::InsufficientPrivileges => StatusCode::FORBIDDEN,
        }
    }
}
