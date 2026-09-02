use crate::api;
use crate::api::error::ApiError;
use crate::app::AppState;
use crate::extract::{Form, HxRequest, Path};
use crate::resource::NotRequested;
use axum::Router;
use axum::extract::{FromRequest, FromRequestParts, Request};
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, VARY};
use axum::response::{Html as AxumHtml, IntoResponse, Redirect, Response};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use serde::Serialize;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use thiserror::Error;
use tower_http::services::ServeDir;

mod comment;
mod form;
mod help;
mod home;
mod login;
mod middleware;
mod pager;
mod pool;
mod pool_category;
mod post;
mod settings;
mod snapshot;
mod tag;
mod tag_category;
mod upload;
mod user;

pub fn post_url<T: Serialize>(post_id: i64, params: &T) -> Result<String, serde_urlencoded::ser::Error> {
    let base = format!("/post/{post_id}");
    url(&base, params)
}

pub fn routes(state: AppState) -> Router {
    // TODO: Remove
    dotenvy::from_filename("../.env").unwrap();
    let data_dir = std::env::var("MOUNT_DATA").unwrap();
    let static_dir = format!("{PROJECT_ROOT}/static");

    help::routes()
        .merge(comment::routes())
        .merge(home::routes())
        .merge(login::routes())
        .merge(pool::routes())
        .merge(pool_category::routes())
        .merge(post::routes())
        .merge(settings::routes())
        .merge(snapshot::routes())
        .merge(tag::routes())
        .merge(tag_category::routes())
        .merge(upload::routes())
        .merge(user::routes())
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), middleware::convert_error))
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), api::middleware::auth))
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), middleware::slow))
        .nest_service("/data", ServeDir::new(&data_dir))
        .nest_service("/static", ServeDir::new(&static_dir))
        .with_state(state)
}

type WebResult<T> = Result<T, WebError>;

const REDIRECT_COOKIE_VALUE: &str = "success";

const PROJECT_ROOT: &str = env!("CARGO_MANIFEST_DIR");

#[derive(PartialEq, Eq)]
enum Tab {
    Home,
    Post,
    Upload,
    Comment,
    Tag,
    Pool,
    User,
    Account,
    Login,
    Help,
    Settings,
    None,
}

struct Html(String);

impl IntoResponse for Html {
    fn into_response(self) -> Response {
        ([(CACHE_CONTROL, "no-store"), (VARY, "HX-Request")], AxumHtml(self.0)).into_response()
    }
}

#[derive(Debug, Error)]
#[error(transparent)]
enum WebError {
    Api(#[from] crate::api::error::ApiError),
    Template(#[from] askama::Error),
}

impl WebError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Api(err) => err.status_code(),
            Self::Template(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<NotRequested> for WebError {
    fn from(value: NotRequested) -> Self {
        WebError::Template(value.into())
    }
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        let mut response = self.status_code().into_response();
        response.extensions_mut().insert(Arc::new(self));
        response
    }
}

fn url<T: Serialize>(base: &str, params: &T) -> Result<String, serde_urlencoded::ser::Error> {
    serde_urlencoded::to_string(params).map(|query_string| {
        if query_string.is_empty() {
            base.to_owned()
        } else {
            format!("{base}?{query_string}")
        }
    })
}

enum Message {
    None,
    Success,
    Error(ApiError),
}

struct PathForm<P, F> {
    path: P,
    form: F,
}

impl<P: Clone, F> PathForm<P, F> {
    pub fn path(&self) -> Path<P> {
        Path(self.path.clone())
    }
}

impl<P, F> Deref for PathForm<P, F> {
    type Target = F;
    fn deref(&self) -> &Self::Target {
        &self.form
    }
}

impl<P, F> DerefMut for PathForm<P, F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.form
    }
}

impl<S, P, F> FromRequest<S> for PathForm<P, F>
where
    S: Send + Sync,
    P: Send,
    Path<P>: FromRequestParts<S, Rejection = ApiError>,
    Form<F>: FromRequest<S, Rejection = ApiError>,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let (mut parts, body) = req.into_parts();
        let Path(path) = Path::from_request_parts(&mut parts, state).await?;

        let req = Request::from_parts(parts, body);
        let Form(form) = Form::from_request(req, state).await?;
        Ok(Self { path, form })
    }
}

fn redirect(relative_url: &str, hx: &HxRequest, jar: CookieJar) -> Response {
    let flash = Cookie::build(("flash", REDIRECT_COOKIE_VALUE))
        .path("/") // TODO: Adjust to base URL
        .http_only(true)
        .same_site(SameSite::Strict)
        .build();
    let jar = jar.add(flash);

    // TODO: Generate using base URL
    if hx.full_page() {
        (jar, Redirect::to(relative_url)).into_response()
    } else {
        (jar, [("HX-Redirect", relative_url)], "").into_response()
    }
}

fn redirect_message(jar: CookieJar) -> (CookieJar, Message) {
    let message = match jar.get("flash").map(Cookie::value) {
        Some(REDIRECT_COOKIE_VALUE) => Message::Success,
        _ => Message::None,
    };
    (jar.remove(Cookie::build("flash").path("/")), message) // TODO: Adjust to base URL
}
