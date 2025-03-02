use crate::api::{ApiResult, AuthResult};
use crate::content::download;
use crate::content::upload::{self, MAX_UPLOAD_SIZE, PartName};
use crate::{api, config};
use serde::{Deserialize, Serialize};
use url::Url;
use warp::multipart::FormData;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let upload_url = warp::post()
        .and(api::auth())
        .and(warp::path!("uploads"))
        .and(warp::body::json())
        .then(upload_url)
        .map(api::Reply::from);
    let upload_multipart = warp::post()
        .and(api::auth())
        .and(warp::path!("uploads"))
        .and(warp::filters::multipart::form().max_length(MAX_UPLOAD_SIZE))
        .then(upload_multipart)
        .map(api::Reply::from);

    upload_url.or(upload_multipart)
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

async fn upload_url(auth: AuthResult, body: UploadBody) -> ApiResult<UploadResponse> {
    let client = auth?;
    api::verify_privilege(client, config::privileges().upload_create)?;

    let token = download::from_url(body.content_url).await?;
    Ok(UploadResponse { token })
}

async fn upload_multipart(auth: AuthResult, form_data: FormData) -> ApiResult<UploadResponse> {
    let body = upload::extract(form_data, [PartName::Content]).await?;
    if let [Some(upload)] = body.files {
        let client = auth?;
        api::verify_privilege(client, config::privileges().upload_create)?;

        let token = upload.save()?;
        Ok(UploadResponse { token })
    } else if let Some(metadata) = body.metadata {
        let url_upload: UploadBody = serde_json::from_slice(&metadata)?;
        upload_url(auth, url_upload).await
    } else {
        Err(api::Error::MissingFormData)
    }
}
