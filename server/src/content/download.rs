use crate::api::error::ApiResult;
use crate::config::Action;
use crate::content::upload::UploadToken;
use crate::extract::Ctx;
use crate::model::enums::MimeType;
use crate::{content, filesystem};
use mime::Mime;
use reqwest::header::{HeaderMap, HeaderValue, REFERER};
use reqwest::{Client, StatusCode};
use std::str::FromStr;
use url::Url;

/// Attempts to download file at the specified `url`.
/// If successful, the file is saved in the temporary uploads directory
/// and a content token is returned.
pub async fn from_url(ctx: &Ctx, url: Url) -> ApiResult<UploadToken> {
    // Some websites expect a user-agent
    const FAKE_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:135.0) Gecko/20100101 Firefox/135.0";

    ctx.verify_privilege(Action::UploadUseDownloader)?;

    let client = Client::builder().user_agent(FAKE_USER_AGENT).build()?;
    let mut response = client.get(url.clone()).send().await?;
    if response.status() == StatusCode::FORBIDDEN {
        // Add referer, as some websites will 403 without it
        let mut headers = HeaderMap::new();
        headers.insert(REFERER, HeaderValue::from_str(url.as_str())?);

        let client = Client::builder()
            .user_agent(FAKE_USER_AGENT)
            .default_headers(headers)
            .build()?;
        response = client.get(url).send().await?;
    }
    let response = response.error_for_status()?;

    let mime = content::parse_header(response.headers())?;
    let mime_essensce = mime.as_ref().map_or("", Mime::essence_str);
    let mime_type = MimeType::from_str(mime_essensce).map_err(Box::from)?;

    filesystem::save_uploaded_file(&ctx.config, response.bytes_stream(), mime_type).await
}
