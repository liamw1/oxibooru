use crate::api;
use crate::api::ApiResult;
use crate::content::FileContents;
use crate::model::enums::MimeType;
use crate::string::SmallString;
use axum::extract::multipart::{Field, Multipart};
use std::ffi::OsStr;
use std::path::Path;
use std::str::FromStr;
use strum::IntoStaticStr;

pub const MAX_UPLOAD_SIZE: usize = 4 * 1024_usize.pow(3);

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

/// Attempts to extract given `fields` and optional JSON "metadata" field from given `form_data`.
pub async fn extract<const N: usize>(mut form_data: Multipart, fields: [PartName; N]) -> ApiResult<Body<N>> {
    let mut files = std::array::from_fn(|_| None);
    let mut metadata = None;
    while let Some(field) = form_data.next_field().await? {
        let position = fields
            .iter()
            .map(Into::<&str>::into)
            .position(|name| field.name() == Some(name));
        if position.is_none() && field.name() != Some("metadata") {
            continue;
        }

        // Get MIME type from field
        let file_info = position
            .map(|index| get_mime_type(&field).map(|mime_type| (index, mime_type)))
            .transpose()?;

        // Ensure metadata is JSON
        if file_info.is_none() && field.content_type() != Some("application/json") {
            return Err(api::Error::InvalidMetadataType);
        }

        let data = field.bytes().await.map_err(api::Error::from)?.to_vec();
        match file_info {
            Some((index, mime_type)) => files[index] = Some(FileContents { data, mime_type }),
            None => metadata = Some(data),
        }
    }
    Ok(Body { files, metadata })
}

/// Returns the MIME type of the given part.
/// It either gets this from the filename extension or the content type if no extension exists.
/// If both exist but their content types are different, an error is returned.
fn get_mime_type(field: &Field) -> ApiResult<MimeType> {
    let extension = field
        .file_name()
        .map(Path::new)
        .and_then(Path::extension)
        .and_then(OsStr::to_str);
    let content_type = field.content_type().map(str::trim);

    match (extension, content_type) {
        (Some(ext), None | Some("application/octet-stream")) => MimeType::from_extension(ext).map_err(api::Error::from),
        (Some(ext), Some(content_type)) => {
            let mime_type = MimeType::from_extension(ext)?;
            if MimeType::from_str(content_type) != Ok(mime_type) {
                return Err(api::Error::ContentTypeMismatch(mime_type, SmallString::new(content_type)));
            }
            Ok(mime_type)
        }
        (None, Some(content_type)) => MimeType::from_str(content_type)
            .map_err(Box::from)
            .map_err(api::Error::from),
        (None, None) => Err(api::Error::MissingContentType),
    }
}
