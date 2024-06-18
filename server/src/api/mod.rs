pub mod info;
pub mod tag_category;

use thiserror::Error;
use warp::http::StatusCode;
use warp::reject::Rejection;
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
    fn to_reply(self) -> Result<WithStatus<String>, Rejection> {
        match self {
            ApiError::FailedConnection(err) => {
                Ok(warp::reply::with_status(err.to_string(), StatusCode::SERVICE_UNAVAILABLE))
            }
            ApiError::FailedQuery(err) => {
                Ok(warp::reply::with_status(err.to_string(), StatusCode::INTERNAL_SERVER_ERROR))
            }
            ApiError::InsufficientPrivileges => Err(warp::reject()),
        }
    }
}
