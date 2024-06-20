pub mod info;
pub mod pool_category;
pub mod tag_category;
pub mod user;

use crate::model::rank::UserRank;
use serde::Serialize;
use thiserror::Error;
use warp::http::StatusCode;
use warp::reply::Json;
use warp::reply::Response;
use warp::reply::WithStatus;
use warp::Filter;

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

pub fn routes() -> impl Filter<Extract = impl warp::Reply, Error = std::convert::Infallible> + Clone {
    let auth = warp::header::optional("Authorization").map(|_token: Option<String>| UserRank::Anonymous);
    let log = warp::filters::log::custom(|info| println!("{} {} [{}]", info.method(), info.path(), info.status()));

    let get_info = warp::get().and(warp::path!("info")).and_then(info::get_info);
    let list_tag_categories = warp::get()
        .and(warp::path!("tag-categories"))
        .and(auth)
        .and_then(tag_category::list_tag_categories);
    let list_pool_categories = warp::get()
        .and(warp::path!("pool-categories"))
        .and(auth)
        .and_then(pool_category::list_pool_categories);
    let post_user = warp::post()
        .and(warp::path!("users"))
        .and(warp::body::bytes())
        .and_then(user::post_user);

    let catch_all = warp::any().map(|| {
        println!("Unimplemented request!");
        warp::reply::with_status("Bad Request", StatusCode::BAD_REQUEST)
    });

    get_info
        .or(list_tag_categories)
        .or(list_pool_categories)
        .or(post_user)
        .or(catch_all)
        .with(log)
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
