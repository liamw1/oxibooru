use crate::api;
use crate::api::ApiResult;
use crate::content::FileContents;
use crate::model::enums::MimeType;
use futures::{StreamExt, TryStreamExt};
use std::ffi::OsStr;
use std::path::Path;
use std::str::FromStr;
use strum::IntoStaticStr;
use warp::filters::multipart::Part;
use warp::multipart::FormData;
use warp::Buf;

pub const MAX_UPLOAD_SIZE: u64 = 4 * 1024_u64.pow(3);

#[derive(Clone, Copy, PartialEq, Eq, IntoStaticStr)]
#[strum(serialize_all = "lowercase")]
pub enum PartName {
    Content,
    Thumbnail,
    Avatar,
}

pub struct Body<const N: usize> {
    pub files: [Option<FileContents>; N],
    pub metadata: Option<Vec<u8>>,
}

pub async fn extract_with_metadata<const N: usize>(form_data: FormData, parts: [PartName; N]) -> ApiResult<Body<N>> {
    extract(form_data, parts, true).await
}

pub async fn extract_without_metadata<const N: usize>(form_data: FormData, parts: [PartName; N]) -> ApiResult<Body<N>> {
    extract(form_data, parts, false).await
}

async fn extract<const N: usize>(
    mut form_data: FormData,
    parts: [PartName; N],
    extract_metadata: bool,
) -> ApiResult<Body<N>> {
    let mut files = std::array::from_fn(|_| None);
    let mut metadata = None;
    while let Some(Ok(part)) = form_data.next().await {
        let position = parts
            .iter()
            .map(Into::<&str>::into)
            .position(|name| part.name() == name);
        if position.is_none() && (!extract_metadata || part.name() != "metadata") {
            continue;
        }

        // Get MIME type from part
        let file_info = position
            .map(|index| get_mime_type(&part).map(|mime_type| (index, mime_type)))
            .transpose()?;

        // Ensure metadata is JSON
        if file_info.is_none() && part.content_type() != Some("application/json") {
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
            Some((index, mime_type)) => files[index] = Some(FileContents { data, mime_type }),
            None => metadata = Some(data),
        };
    }
    Ok(Body { files, metadata })
}

/// Returns the MIME type of the given part.
/// It either gets this from the filename extension or the content type if no extension exists.
/// If both exist but their content types are different, an error is returned.
fn get_mime_type(part: &Part) -> ApiResult<MimeType> {
    let extension = part
        .filename()
        .map(Path::new)
        .and_then(Path::extension)
        .and_then(OsStr::to_str);
    let content_type = part.content_type().map(str::trim);

    match (extension, content_type) {
        (Some(ext), None) | (Some(ext), Some("application/octet-stream")) => {
            MimeType::from_extension(ext).map_err(api::Error::from)
        }
        (Some(ext), Some(content_type)) => {
            let mime_type = MimeType::from_extension(ext)?;
            if MimeType::from_str(content_type) != Ok(mime_type) {
                return Err(api::Error::ContentTypeMismatch(mime_type, content_type.to_owned()));
            }
            Ok(mime_type)
        }
        (None, Some(content_type)) => MimeType::from_str(content_type)
            .map_err(Box::from)
            .map_err(api::Error::from),
        (None, None) => Err(api::Error::MissingContentType),
    }
}
