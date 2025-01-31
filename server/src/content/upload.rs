use crate::api;
use crate::api::ApiResult;
use crate::model::enums::MimeType;
use futures::{StreamExt, TryStreamExt};
use std::str::FromStr;
use strum::IntoStaticStr;
use warp::multipart::FormData;
use warp::Buf;

#[derive(Clone, Copy, IntoStaticStr)]
#[strum(serialize_all = "lowercase")]
pub enum Part {
    Content,
    Thumbnail,
    Avatar,
    Metadata,
}

pub struct Upload {
    pub data: Vec<u8>,
    pub mime_type: MimeType,
}

pub async fn extract<const N: usize>(mut form_data: FormData, parts: [Part; N]) -> ApiResult<[Option<Upload>; N]> {
    let mut uploads = std::array::from_fn(|_| None);
    while let Some(Ok(part)) = form_data.next().await {
        let position = parts
            .iter()
            .map(Into::<&str>::into)
            .position(|name| part.name() == name);
        let index = match position {
            Some(index) => index,
            None => continue,
        };

        // Ensure file extension matches content type
        let mime_type = MimeType::from_str(part.content_type().unwrap_or("")).map_err(Box::from)?;
        let filename = std::path::Path::new(part.filename().unwrap_or(""));
        let extension = filename.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        if MimeType::from_extension(extension) != Ok(mime_type) {
            return Err(api::Error::ContentTypeMismatch(mime_type, extension.to_owned()));
        }

        let data = part
            .stream()
            .try_fold(Vec::new(), |mut acc, buf| async move {
                acc.extend_from_slice(buf.chunk());
                Ok(acc)
            })
            .await
            .map_err(api::Error::from)?;
        uploads[index] = Some(Upload { data, mime_type });
    }
    Ok(uploads)
}
