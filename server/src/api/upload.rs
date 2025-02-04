use crate::api::{ApiResult, AuthResult};
use crate::content::upload::{self, PartName, MAX_UPLOAD_SIZE};
use crate::{api, config, filesystem};
use serde::Serialize;
use warp::multipart::FormData;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::post()
        .and(api::auth())
        .and(warp::path!("uploads"))
        .and(warp::filters::multipart::form().max_length(MAX_UPLOAD_SIZE))
        .then(upload)
        .map(api::Reply::from)
}

#[derive(Serialize)]
struct UploadResponse {
    token: String,
}

async fn upload(auth: AuthResult, form_data: FormData) -> ApiResult<UploadResponse> {
    let client = auth?;
    api::verify_privilege(client, config::privileges().upload_create)?;

    let body = upload::extract_without_metadata(form_data, [PartName::Content]).await?;
    if let [Some(upload)] = body.files {
        let token = filesystem::save_uploaded_file(&upload.data, upload.content_type)?;
        Ok(UploadResponse { token })
    } else {
        Err(api::Error::MissingFormData)
    }
}
