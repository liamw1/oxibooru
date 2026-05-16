use crate::api::error::ApiError;
use crate::app::{AppState, Context};
use crate::db::AsyncConnectionPool;
use crate::model::enums::Rating;
use crate::resource::field::Mask;
use crate::time::DateTime;
use axum::RequestPartsExt;
use axum::extract::multipart::{Multipart as AxumMultipart, MultipartRejection};
use axum::extract::rejection::{JsonRejection, MissingJsonContentType, PathRejection, QueryRejection};
use axum::extract::{
    Extension, FromRef, FromRequest, FromRequestParts, Json as AxumJson, Path as AxumPath, Query as AxumQuery, Request,
    State,
};
use axum::http::header::CONTENT_TYPE;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use std::num::NonZeroI64;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

#[derive(Clone)]
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

/// Request body to apply/change a score.
#[derive(Deserialize, ToSchema)]
pub struct RatingBody {
    pub score: Rating,
}

impl Deref for RatingBody {
    type Target = Rating;
    fn deref(&self) -> &Self::Target {
        &self.score
    }
}

/// Request body for deleting a resource.
#[derive(Deserialize, ToSchema)]
pub struct DeleteBody {
    /// Resource version. See [versioning](#Versioning).
    pub version: DateTime,
}

impl Deref for DeleteBody {
    type Target = DateTime;
    fn deref(&self) -> &Self::Target {
        &self.version
    }
}

/// Request body for merging resources.
#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MergeBody<T> {
    /// ID of the source resource to be removed.
    pub remove: T,
    /// ID of the target resource to merge into.
    pub merge_to: T,
    /// Version of the source resource. See [versioning](#Versioning).
    pub remove_version: DateTime,
    /// Version of the target resource. See [versioning](#Versioning).
    pub merge_to_version: DateTime,
}

/// Represents parameters of a request to retrieve one or more resources.
#[derive(Deserialize, IntoParams)]
#[serde(bound(deserialize = "F: Into<u64> + FromStr"))]
pub struct ResourceParams<F: Into<u64> + FromStr> {
    /// Query search string
    #[param(example = "anonymous_token")]
    pub query: Option<String>,
    /// Comma-separated list of fields to include in the response. See [field selection](#Field-Selection) for details.
    #[param(value_type = Option<String>, example = "field1,field2")]
    pub fields: Mask<F>,
}

impl<F: Into<u64> + FromStr> ResourceParams<F> {
    pub fn criteria(&self) -> &str {
        self.query.as_deref().unwrap_or("")
    }
}
/// Represents parameters of a request to retrieve multiple resources, paged.
#[derive(Deserialize, IntoParams)]
pub struct PageParams {
    /// Starting position in the result set
    #[param(example = 0)]
    pub offset: Option<i64>,
    /// Maximum number of results to return
    #[param(value_type = i64, minimum = 1, example = 40)]
    limit: Option<NonZeroI64>,
}

impl PageParams {
    pub fn limit(&self) -> i64 {
        const DEFAULT_LIMIT: i64 = 40;
        const MAX_LIMIT: i64 = 1000;
        std::cmp::min(self.limit.map(NonZeroI64::get).unwrap_or(DEFAULT_LIMIT), MAX_LIMIT)
    }
}

/// A result of search operation that doesn't involve paging.
#[derive(Serialize, ToSchema)]
pub struct UnpagedResponse<T> {
    pub results: Vec<T>,
}

/// A result of search operation that involves paging.
#[derive(Serialize, ToSchema)]
pub struct PagedResponse<T> {
    /// The query passed in the original request that contains a standard [search query](#Search).
    #[schema(examples("anonymous_token named_token:value1,value2,value3 sort:sort_token"))]
    pub query: Option<String>,
    /// The record starting offset, passed in the original request.
    #[schema(examples(0))]
    pub offset: i64,
    /// Number of records on one page.
    #[schema(examples(40))]
    pub limit: i64,
    /// How many resources were found. To get the page count, divide this number by `limit`.
    #[schema(examples(1729))]
    pub total: i64,
    pub results: Vec<T>,
}
