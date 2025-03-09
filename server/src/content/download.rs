use crate::api::ApiResult;
use crate::model::enums::MimeType;
use crate::{api, filesystem};
use reqwest::Client;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue, REFERER};
use std::str::FromStr;
use url::Url;

/// Attempts to download file at the specified `url`.
/// If successful, the file is saved in the temporary uploads directory
/// and a content token is returned.
pub async fn from_url(url: Url) -> ApiResult<String> {
    // Some websites expect a user-agent
    const FAKE_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:135.0) Gecko/20100101 Firefox/135.0";

    // Add referer, as some websites will 403 without it
    let mut headers = HeaderMap::new();
    headers.insert(REFERER, HeaderValue::from_str(url.as_str())?);

    let client = Client::builder()
        .user_agent(FAKE_USER_AGENT)
        .default_headers(headers)
        .build()?;
    let response = client.get(url).send().await?;
    let response = response.error_for_status()?;

    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .map(|header_value| header_value.to_str())
        .transpose()?;
    let mime_type = MimeType::from_str(content_type.unwrap_or("")).map_err(Box::from)?;

    let bytes = response.bytes().await?;
    filesystem::save_uploaded_file(&bytes, mime_type).map_err(api::Error::from)
}
