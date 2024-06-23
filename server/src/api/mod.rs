pub mod info;
pub mod pool_category;
pub mod tag_category;
pub mod user;
pub mod user_token;

use crate::auth::header::{self, AuthenticationError};
use crate::error::ErrorKind;
use crate::model::rank::UserRank;
use crate::model::user::User;
use serde::{Deserialize, Serialize};
use warp::http::StatusCode;
use warp::reply::Json;
use warp::reply::Response;
use warp::reply::WithStatus;
use warp::Filter;

/*
    NOTE: In general, it seems like we do not send the id of a resource back to the
          client. Perhaps we should consider doing this as then we could query
          user, tags, etc. by their primary key. We could also use #[serde(flatten)]
          to use the actual resource structs in the serialize structs.
*/

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
    BadHash(#[from] crate::auth::HashError),
    BadHeader(#[from] warp::http::header::ToStrError),
    BadUserPrivilege(#[from] crate::model::rank::ParseUserPrivilegeError),
    FailedAuthentication(#[from] AuthenticationError),
    FailedConnection(#[from] diesel::ConnectionError),
    FailedQuery(#[from] diesel::result::Error),
    #[error("Insufficient privileges")]
    InsufficientPrivileges,
}

impl Error {
    fn status_code(&self) -> StatusCode {
        type QueryError = diesel::result::Error;

        let query_error_status_code = |err: &QueryError| match err {
            QueryError::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        match self {
            Error::BadBody(_) => StatusCode::BAD_REQUEST,
            Error::BadHash(_) => StatusCode::BAD_REQUEST,
            Error::BadHeader(_) => StatusCode::BAD_REQUEST,
            Error::BadUserPrivilege(_) => StatusCode::BAD_REQUEST,
            Error::FailedAuthentication(err) => match err {
                AuthenticationError::FailedConnection(_) => StatusCode::SERVICE_UNAVAILABLE,
                AuthenticationError::FailedQuery(err) => query_error_status_code(err),
                _ => StatusCode::UNAUTHORIZED,
            },
            Error::FailedConnection(_) => StatusCode::SERVICE_UNAVAILABLE,
            Error::FailedQuery(err) => query_error_status_code(err),
            Error::InsufficientPrivileges => StatusCode::FORBIDDEN,
        }
    }

    fn category(&self) -> &'static str {
        match self {
            Error::BadBody(_) => "Bad Body",
            Error::BadHash(_) => "Bad Hash",
            Error::BadHeader(_) => "Bad Header",
            Error::BadUserPrivilege(_) => "Bad User Privilege",
            Error::FailedAuthentication(_) => "Failed Authentication",
            Error::FailedConnection(_) => "Failed Connection",
            Error::FailedQuery(_) => "Failed Query",
            Error::InsufficientPrivileges => "Insufficient Privileges",
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
            Self::BadHash(err) => err.kind(),
            Self::BadHeader(_) => "BadHeader",
            Self::BadUserPrivilege(_) => "BadUserPrivilege",
            Self::FailedAuthentication(err) => err.kind(),
            Self::FailedConnection(err) => err.kind(),
            Self::FailedQuery(err) => err.kind(),
            Self::InsufficientPrivileges => "InsufficientPrivileges",
        }
    }
}

pub fn routes() -> impl Filter<Extract = impl warp::Reply, Error = std::convert::Infallible> + Clone {
    let auth = warp::header::optional("Authorization").map(|opt_auth: Option<_>| {
        opt_auth
            .map(|auth| header::authenticate_user(auth).map(Some))
            .unwrap_or(Ok(None))
            .map_err(Error::from)
    });
    let log = warp::filters::log::custom(|info| {
        // println!("Header: {:?}", info.request_headers());
        println!("{} {} [{}]", info.method(), info.path(), info.status());
    });

    let get_info = warp::get().and(warp::path!("info")).and_then(info::get_info);
    let list_tag_categories = warp::get()
        .and(warp::path!("tag-categories"))
        .and(auth)
        .and_then(tag_category::list_tag_categories);
    let list_pool_categories = warp::get()
        .and(warp::path!("pool-categories"))
        .and(auth)
        .and_then(pool_category::list_pool_categories);
    let get_user = warp::get()
        .and(warp::path!("user" / String))
        .and(auth)
        .and_then(user::get_user);
    let post_user = warp::post()
        .and(warp::path!("users"))
        .and(auth)
        .and(warp::body::bytes())
        .and_then(user::post_user);
    let post_user_token = warp::post()
        .and(warp::path!("user-token" / String))
        .and(auth)
        .and(warp::body::bytes())
        .and_then(user_token::post_user);

    let catch_all = warp::any().map(|| {
        println!("Unimplemented request!");
        warp::reply::with_status("Bad Request", StatusCode::BAD_REQUEST)
    });

    get_info
        .or(list_tag_categories)
        .or(list_pool_categories)
        .or(get_user)
        .or(post_user)
        .or(post_user_token)
        .or(catch_all)
        .with(log)
}

type AuthenticationResult = Result<Option<User>, Error>;

#[derive(Serialize)]
struct ErrorResponse {
    title: String,
    name: String,
    description: String,
}

#[derive(Serialize, Deserialize)]
struct MicroUser {
    name: String,
    #[serde(rename(serialize = "avatarUrl", deserialize = "avatarUrl"))]
    avatar_url: String,
}

impl MicroUser {
    fn new(user: User) -> Self {
        let avatar_url = user.avatar_url();
        Self {
            name: user.name,
            avatar_url,
        }
    }
}

fn client_access_level(client: Option<&User>) -> UserRank {
    client.map(|user| user.rank).unwrap_or(UserRank::Anonymous)
}

fn access_level(auth_result: AuthenticationResult) -> Result<UserRank, Error> {
    auth_result.map(|client| client_access_level(client.as_ref()))
}

fn validate_privilege(client_rank: UserRank, requested_action: &str) -> Result<(), Error> {
    if !client_rank.has_permission_to(requested_action) {
        return Err(Error::InsufficientPrivileges);
    }
    Ok(())
}

/*
    For some reason warp::body::json rejects incoming requests, perhaps due to encoding
    issues. Instead, we will parse the raw bytes into a deserialize-capable structure.
*/
fn parse_body<'a, T: serde::Deserialize<'a>>(body: &'a [u8]) -> Result<T, Error> {
    println!("Incoming body: {}", std::str::from_utf8(body).unwrap_or("ERROR: Failed to parse"));
    serde_json::from_slice(body).map_err(Error::from)
}
