use crate::api::{ApiError, ApiResult};
use crate::auth::Client;
use crate::content::upload::{self, MAX_UPLOAD_SIZE, PartName};
use crate::content::{JsonOrMultipart, download};
use crate::{api, config};
use axum::extract::{DefaultBodyLimit, Extension};
use axum::{Json, Router, routing};
use serde::{Deserialize, Serialize};
use url::Url;

pub fn routes() -> Router {
    Router::new()
        .route("/uploads", routing::post(upload_handler))
        .route_layer(DefaultBodyLimit::max(MAX_UPLOAD_SIZE))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct UploadBody {
    content_url: Url,
}

#[derive(Serialize)]
struct UploadResponse {
    token: String,
}

/// See [uploading-temporary-file](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#uploading-temporary-file)
async fn upload_from_url(body: UploadBody) -> ApiResult<Json<UploadResponse>> {
    let token = download::from_url(body.content_url).await?;
    Ok(Json(UploadResponse { token }))
}

/// See [uploading-temporary-file](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#uploading-temporary-file)
async fn upload_handler(
    Extension(client): Extension<Client>,
    body: JsonOrMultipart<UploadBody>,
) -> ApiResult<Json<UploadResponse>> {
    api::verify_privilege(client, config::privileges().upload_create)?;

    match body {
        JsonOrMultipart::Json(payload) => upload_from_url(payload).await,
        JsonOrMultipart::Multipart(payload) => {
            let decoded_body = upload::extract(payload, [PartName::Content]).await?;
            if let [Some(upload)] = decoded_body.files {
                let token = upload.save()?;
                Ok(Json(UploadResponse { token }))
            } else if let Some(metadata) = decoded_body.metadata {
                let url_upload: UploadBody = serde_json::from_slice(&metadata)?;
                upload_from_url(url_upload).await
            } else {
                Err(ApiError::MissingFormData)
            }
        }
    }
}
