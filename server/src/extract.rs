use crate::api::error::ApiError;
use crate::app::{AppState, Context};
use crate::content;
use crate::db::AsyncConnectionPool;
use axum::RequestPartsExt;
use axum::extract::multipart::{Multipart as AxumMultipart, MultipartRejection};
use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::extract::{
    Extension, FromRef, FromRequest, FromRequestParts, Json as AxumJson, Path as AxumPath, Query as AxumQuery, Request,
    State,
};
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use mime::{APPLICATION, FORM_DATA, JSON, MULTIPART};
use serde::Serialize;
use std::ops::Deref;
use std::sync::Arc;

pub struct Ctx(pub Context, pub Arc<AsyncConnectionPool>);

impl Deref for Ctx {
    type Target = Context;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<S> FromRequestParts<S> for Ctx
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Ok(State(state)) = State::<AppState>::from_request_parts(parts, state).await;
        let Extension(client) = parts.extract().await.map_err(ApiError::from)?;
        Ok(state.make_context(client))
    }
}

// Wrappers over fallible extensions to provide error handling.

pub struct Json<T>(pub T);

impl<S, T> FromRequest<S> for Json<T>
where
    AxumJson<T>: FromRequest<S, Rejection = JsonRejection>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        AxumJson::<T>::from_request(req, state)
            .await
            .map(|value| Self(value.0))
            .map_err(ApiError::from)
    }
}

impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        AxumJson(self.0).into_response()
    }
}

/// Used for bodies which can either be expressed as JSON or Multipart form, like uploads.
pub enum JsonOrMultipart<T> {
    Json(T),
    Multipart(AxumMultipart),
}

impl<S, T> FromRequest<S> for JsonOrMultipart<T>
where
    AxumJson<T>: FromRequest<S, Rejection = JsonRejection>,
    AxumMultipart: FromRequest<S, Rejection = MultipartRejection>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        if let Some(mime) = content::parse_header(req.headers())? {
            if mime.type_() == APPLICATION && (mime.subtype() == JSON || mime.suffix() == Some(JSON)) {
                return AxumJson::from_request(req, state)
                    .await
                    .map(|AxumJson(value)| Self::Json(value))
                    .map_err(ApiError::from);
            } else if mime.type_() == MULTIPART && mime.subtype() == FORM_DATA {
                return AxumMultipart::from_request(req, state)
                    .await
                    .map(Self::Multipart)
                    .map_err(ApiError::from);
            }
        }
        Err(ApiError::UnsupportedContentType)
    }
}

pub struct Path<T>(pub T);

impl<S, T> FromRequestParts<S> for Path<T>
where
    AxumPath<T>: FromRequestParts<S, Rejection = PathRejection>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        AxumPath::from_request_parts(parts, state)
            .await
            .map(|value| Self(value.0))
            .map_err(ApiError::from)
    }
}

pub struct Query<T>(pub T);

impl<S, T> FromRequestParts<S> for Query<T>
where
    AxumQuery<T>: FromRequestParts<S, Rejection = QueryRejection>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        AxumQuery::from_request_parts(parts, state)
            .await
            .map(|value| Self(value.0))
            .map_err(ApiError::from)
    }
}
