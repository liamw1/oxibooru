use crate::api;
use crate::api::ApiResult;
use crate::model::enums::MimeType;
use futures::{StreamExt, TryStreamExt};
use std::str::FromStr;
use strum::IntoStaticStr;
use warp::multipart::FormData;
use warp::Buf;

pub const MAX_UPLOAD_SIZE: u64 = 4 * 1024_u64.pow(3);

#[derive(Clone, Copy, PartialEq, Eq, IntoStaticStr)]
#[strum(serialize_all = "lowercase")]
pub enum Part {
    Content,
    Thumbnail,
    Avatar,
}

pub struct File {
    pub data: Vec<u8>,
    pub content_type: MimeType,
}

pub struct Upload<const N: usize> {
    pub files: [Option<File>; N],
    pub metadata: Option<Vec<u8>>,
}

pub async fn extract<const N: usize>(mut form_data: FormData, parts: [Part; N]) -> ApiResult<Upload<N>> {
    let mut files = std::array::from_fn(|_| None);
    let mut metadata = None;
    while let Some(Ok(part)) = form_data.next().await {
        let position = parts
            .iter()
            .map(Into::<&str>::into)
            .position(|name| part.name() == name);
        if position.is_none() && part.name() != "metadata" {
            continue;
        }
        let file_info = position
            .map(|index| MimeType::from_str(part.content_type().unwrap_or("")).map(|mime_type| (index, mime_type)))
            .transpose()
            .map_err(Box::from)?;

        // Ensure file extension matches content type
        if let Some((_, mime_type)) = file_info {
            let filename = std::path::Path::new(part.filename().unwrap_or(""));
            let extension = filename.extension().and_then(|ext| ext.to_str()).unwrap_or("");
            if MimeType::from_extension(extension) != Ok(mime_type) {
                return Err(api::Error::ContentTypeMismatch(mime_type, extension.to_owned()));
            }
        } else if part.content_type() != Some("application/json") {
            return Err(api::Error::InvalidMetadataType);
        }

        let data = part
            .stream()
            .try_fold(Vec::new(), |mut acc, buf| async move {
                acc.extend_from_slice(buf.chunk());
                Ok(acc)
            })
            .await
            .map_err(api::Error::from)?;
        match file_info {
            Some((index, content_type)) => files[index] = Some(File { data, content_type }),
            None => metadata = Some(data),
        };
    }
    Ok(Upload { files, metadata })
}
