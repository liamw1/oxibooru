use crate::api::ApiResult;
use crate::content::cache::CachedProperties;
use crate::content::thumbnail::ThumbnailType;
use crate::content::{cache, decode, thumbnail};
use crate::model::enums::MimeType;
use crate::{api, filesystem};
use futures::{StreamExt, TryStreamExt};
use image::DynamicImage;
use serde::de::Visitor;
use serde::{Deserialize, Deserializer};
use std::ffi::OsStr;
use std::path::Path;
use std::str::FromStr;
use strum::IntoStaticStr;
use warp::filters::multipart::Part;
use warp::multipart::FormData;
use warp::Buf;

pub const MAX_UPLOAD_SIZE: u64 = 4 * 1024_u64.pow(3);

/// Stores file contents and content type of an uploaded file.
pub struct FileContents {
    pub data: Vec<u8>,
    pub content_type: MimeType,
}

impl FileContents {
    /// Constructs an instance from a temporary upload.
    pub fn from_token(token: &str) -> ApiResult<Self> {
        let (_uuid, extension) = token.split_once('.').unwrap();
        let content_type = MimeType::from_extension(extension)?;

        let temp_path = filesystem::temporary_upload_filepath(token);
        let data = std::fs::read(&temp_path)?;

        Ok(Self { data, content_type })
    }
}

/// Contains either the name of a file uploaded to the temporary uploads
/// directory or the contents of the file sent via a multipart request.
///
/// Only the Token variant can be deserialized. The Content variant has
/// to be set manually.
pub enum Upload {
    Token(String),
    Content(FileContents),
}

impl Upload {
    pub fn thumbnail(&self, thumbnail_type: ThumbnailType) -> ApiResult<DynamicImage> {
        let file_contents = match self {
            Self::Token(token) => &FileContents::from_token(token)?,
            Self::Content(contents) => contents,
        };
        decode::representative_image(&file_contents.data, None, file_contents.content_type)
            .map(|image| thumbnail::create(&image, thumbnail_type))
    }

    pub fn compute_properties(&self) -> ApiResult<CachedProperties> {
        match self {
            Self::Token(token) => cache::compute_properties(token.to_owned()),
            Self::Content(file) => {
                let token = filesystem::save_uploaded_file(&file.data, file.content_type)?;
                cache::compute_properties(token)
            }
        }
    }

    pub fn get_or_compute_properties(&self) -> ApiResult<CachedProperties> {
        match self {
            Self::Token(token) => cache::get_or_compute_properties(token.to_owned()),
            Self::Content(file) => {
                let token = filesystem::save_uploaded_file(&file.data, file.content_type)?;
                cache::compute_properties(token)
            }
        }
    }
}

impl<'de> Deserialize<'de> for Upload {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct UploadVisitor;
        impl Visitor<'_> for UploadVisitor {
            type Value = Upload;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string")
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(Upload::Token(value.to_owned()))
            }

            fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(Upload::Token(value))
            }
        }
        deserializer.deserialize_string(UploadVisitor)
    }
}

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
            Some((index, content_type)) => files[index] = Some(FileContents { data, content_type }),
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
