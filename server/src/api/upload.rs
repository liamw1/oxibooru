use crate::api::{ApiResult, AuthResult};
use crate::content::upload::{self, Part};
use crate::filesystem::Directory;
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

const MAX_UPLOAD_SIZE: u64 = 4 * 1024_u64.pow(3);

#[derive(Serialize)]
struct UploadResponse {
    token: String,
}

async fn upload(auth: AuthResult, form_data: FormData) -> ApiResult<UploadResponse> {
    let client = auth?;
    api::verify_privilege(client, config::privileges().upload_create)?;

    // Set up temp directory if necessary
    filesystem::create_dir(Directory::TemporaryUploads)?;

    if let [Some(content)] = upload::extract(form_data, [Part::Content]).await? {
        let upload_token = filesystem::save_uploaded_file(content.data, content.mime_type)?;
        Ok(UploadResponse { token: upload_token })
    } else {
        Err(api::Error::MissingFormData)
    }
}
