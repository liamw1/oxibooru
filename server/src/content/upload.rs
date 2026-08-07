use crate::api::error::{ApiError, ApiResult};
use crate::config::Config;
use crate::filesystem::{self, Directory};
use crate::model::enums::MimeType;
use axum::body::Bytes;
use axum::extract::multipart::Multipart;
use axum::extract::rejection::{JsonRejection, MissingJsonContentType};
use mime::{APPLICATION, JSON, Mime};
use serde::{Deserialize, Deserializer, Serialize};
use std::path::PathBuf;
use std::str::FromStr;
use strum::IntoStaticStr;
use tracing::warn;
use uuid::Uuid;

/// A token that represents a file that's been streamed to disk during upload.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct UploadToken {
    token: String,
    #[serde(skip)]
    mime_type: MimeType,
}

impl UploadToken {
    pub fn new(mime_type: MimeType) -> Self {
        let token = format!("{}.{}", Uuid::new_v4(), mime_type.extension());
        Self { token, mime_type }
    }

    pub fn mime_type(&self) -> MimeType {
        self.mime_type
    }

    pub fn path(&self, config: &Config) -> PathBuf {
        config.path(Directory::TemporaryUploads).join(&self.token)
    }
}

impl<'de> Deserialize<'de> for UploadToken {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let token = String::deserialize(deserializer)?;
        if token.contains('/') || token.contains('\\') || token.contains(':') {
            return Err(serde::de::Error::custom("invalid upload token"));
        }

        let (_uuid, extension) = token.rsplit_once('.').unwrap_or((&token, ""));
        let mime_type = MimeType::from_extension(extension).map_err(serde::de::Error::custom)?;
        Ok(Self { token, mime_type })
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
    pub files: [Option<UploadToken>; N],
    pub metadata: Option<Bytes>,
}

/// Attempts to extract given `fields` and optional JSON "metadata" field from given `form_data`.
pub async fn extract<const N: usize>(
    config: &Config,
    mut form_data: Multipart,
    fields: [PartName; N],
) -> ApiResult<Body<N>> {
    let mut files = std::array::from_fn(|_| None);
    let mut metadata = None;
    while let Some(field) = form_data.next_field().await? {
        let position = fields
            .iter()
            .map(<&str>::from)
            .position(|name| field.name() == Some(name));

        // Skip unexpected fields
        if position.is_none() && field.name() != Some("metadata") {
            if let Some(name) = field.name() {
                warn!("Field `{name}` not expected, skipping");
            } else {
                warn!("No field name specified, skipping");
            }
            continue;
        }

        // Ensure metadata is JSON
        let mime = field.content_type().map(Mime::from_str).transpose()?;
        let is_application_json = mime
            .as_ref()
            .is_some_and(|mime| mime.type_() == APPLICATION && (mime.subtype() == JSON || mime.suffix() == Some(JSON)));
        if position.is_none() && !is_application_json {
            return Err(ApiError::JsonRejection(JsonRejection::MissingJsonContentType(
                MissingJsonContentType::default(),
            )));
        }

        if let Some(index) = position {
            files[index] = filesystem::save_uploaded_file(config, field).await.map(Some)?;
        } else {
            metadata = field.bytes().await.map(Some)?;
        }
    }
    Ok(Body { files, metadata })
}
