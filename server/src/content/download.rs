use crate::api::ApiResult;
use crate::model::enums::MimeType;
use crate::{api, filesystem};
use std::str::FromStr;
use url::Url;

/// Attempts to download file at the specified `url`.
/// If successful, the file is saved in the temporary uploads directory
/// and a content token is returned.
pub async fn from_url(url: Url) -> ApiResult<String> {
    let response = reqwest::get(url).await?;
    let response = response.error_for_status()?;

    let content_type = response
        .headers()
        .get("content-type")
        .map(|header_value| header_value.to_str())
        .transpose()?;
    let mime_type = MimeType::from_str(content_type.unwrap_or("")).map_err(Box::from)?;

    let bytes = response.bytes().await?;
    filesystem::save_uploaded_file(&bytes, mime_type).map_err(api::Error::from)
}
