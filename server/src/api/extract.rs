use crate::api::ApiError;
use axum::extract::multipart::{Multipart as AxumMultipart, MultipartRejection};
use axum::extract::rejection::{JsonRejection, MissingJsonContentType, PathRejection, QueryRejection};
use axum::extract::{FromRequest, FromRequestParts, Json as AxumJson, Path as AxumPath, Query as AxumQuery, Request};
use axum::http::header::CONTENT_TYPE;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

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
        let content_type_header = req.headers().get(CONTENT_TYPE);
        let content_type = content_type_header.map(|value| value.to_str()).transpose()?;

        if let Some(content_type) = content_type {
            if content_type.starts_with("application/json") {
                return AxumJson::<T>::from_request(req, state)
                    .await
                    .map(|value| Self::Json(value.0))
                    .map_err(ApiError::from);
            }
            if content_type.starts_with("multipart/form-data") {
                return AxumMultipart::from_request(req, state)
                    .await
                    .map(Self::Multipart)
                    .map_err(ApiError::from);
            }
        }
        Err(ApiError::JsonRejection(JsonRejection::MissingJsonContentType(MissingJsonContentType::default())))
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
        AxumPath::<T>::from_request_parts(parts, state)
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
