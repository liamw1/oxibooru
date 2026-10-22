use crate::api::error::ApiResult;
use crate::content::upload::UploadToken;
use crate::filesystem::Directory;
use crate::model::enums::{MimeType, PostSafety};
use crate::web::form;
use crate::web::upload::PAUSE_ON_ERROR_DEFAULT;
use serde::{Deserialize, Deserializer};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::ops::Bound;
use std::str::FromStr;
use url::Url;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Add,
    Upload,
    Continue,
    Cancel,
    Remove(usize),
    MoveUp(usize),
    MoveDown(usize),
    Init, // For initializing form: cannot be deserialized into
}

impl FromStr for Operation {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "add" => Some(Self::Add),
            "upload" => Some(Self::Upload),
            "continue" => Some(Self::Continue),
            "cancel" => Some(Self::Cancel),
            _ => {
                if let Some(index) = s.strip_prefix("remove-") {
                    index.parse().map(Self::Remove).ok()
                } else if let Some(index) = s.strip_prefix("move-up-") {
                    index.parse().map(Self::MoveUp).ok()
                } else if let Some(index) = s.strip_prefix("move-down-") {
                    index.parse().map(Self::MoveDown).ok()
                } else {
                    None
                }
            }
        }
        .ok_or("Failed to parse upload operation")
    }
}

impl<'de> Deserialize<'de> for Operation {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = <Cow<str>>::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(PartialEq, Eq)]
pub enum Preview {
    Image,
    Video,
    None,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SimilarPost {
    pub post_id: i64,
    pub thumbnail_url: String,
    pub distance: f64,
    #[serde(default, deserialize_with = "form::checkbox_state")]
    pub copy_tags: bool,
    #[serde(default, deserialize_with = "form::checkbox_state")]
    pub add_relation: bool,
}

impl SimilarPost {
    pub fn match_percent(&self) -> String {
        format!("{:.0}", 100.0 * (1.0 - self.distance))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Content {
    Token(UploadToken),
    Error(String),
    Url(String),
    Held(String),
}

impl From<ApiResult<UploadToken>> for Content {
    fn from(saved_file: ApiResult<UploadToken>) -> Self {
        match saved_file {
            Ok(token) => Self::Token(token),
            Err(err) => Self::Error(err.to_string()),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Uploadable {
    pub name: String,
    pub content: Content,
    pub source: Option<String>,
    #[serde(default)]
    pub safety: PostSafety,
    #[serde(default, deserialize_with = "form::checkbox_state")]
    pub anonymous: bool,
    #[serde(default, deserialize_with = "form::checkbox_state")]
    pub has_similar: bool,
    #[serde(default)]
    pub similar_posts: Vec<SimilarPost>,
    // Also marks the upload as attempted, so that it isn't retried until uploading is restarted
    pub message: Option<String>,
}

impl Uploadable {
    pub fn new(name: String, content: Content, anonymous: bool) -> Self {
        Self {
            name,
            content,
            source: None,
            safety: PostSafety::default(),
            anonymous,
            has_similar: false,
            similar_posts: Vec::new(),
            message: None,
        }
    }

    pub fn preview(&self) -> Preview {
        let mime_type = match &self.content {
            Content::Token(token) => Some(token.mime_type()),
            Content::Url(url) => Url::parse(url).ok().and_then(|url| {
                let (_, extension) = url.path().rsplit_once('.')?;
                MimeType::from_extension(extension).ok()
            }),
            Content::Error(_) | Content::Held(_) => None,
        };
        match mime_type {
            Some(
                MimeType::Avif
                | MimeType::Bmp
                | MimeType::Gif
                | MimeType::Jpeg
                | MimeType::Jxl
                | MimeType::Png
                | MimeType::Webp,
            ) => Preview::Image,
            Some(MimeType::Mov | MimeType::Mp4 | MimeType::Webm) => Preview::Video,
            // TODO: Add preview support for PDF and SWF
            None | Some(MimeType::Pdf | MimeType::Swf) => Preview::None,
        }
    }

    pub fn preview_url(&self) -> String {
        match &self.content {
            Content::Token(token) => format!("data/{}/{token}", Directory::TemporaryUploads),
            Content::Url(url) => url.clone(),
            Content::Error(_) | Content::Held(_) => String::new(),
        }
    }

    pub fn preview_mime_type(&self) -> Option<MimeType> {
        match &self.content {
            Content::Token(token) => Some(token.mime_type()),
            Content::Error(_) | Content::Url(_) | Content::Held(_) => None,
        }
    }
}

#[derive(Deserialize)]
pub struct NewFile {
    pub key: String,
    pub name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct UploadForm {
    pub operation: Operation,
    #[serde(default)]
    pub uploads: BTreeMap<usize, Uploadable>,
    #[serde(default)]
    pub urls: String,
    #[serde(default)]
    pub new_files: Vec<NewFile>,
    #[serde(default, deserialize_with = "form::checkbox_state")]
    pub skip_duplicates: bool,
    #[serde(default, deserialize_with = "form::checkbox_state")]
    pub upload_similar: bool,
    #[serde(default, deserialize_with = "form::checkbox_state")]
    pub pause_on_error: bool,
}

impl UploadForm {
    pub fn initialize() -> Self {
        Self {
            operation: Operation::Init,
            uploads: BTreeMap::new(),
            urls: String::new(),
            new_files: Vec::new(),
            skip_duplicates: false,
            upload_similar: false,
            pause_on_error: PAUSE_ON_ERROR_DEFAULT,
        }
    }

    pub fn push_upload(&mut self, uploadable: Uploadable) {
        let index = self.uploads.keys().last().copied().unwrap_or(0) + 1;
        self.uploads.insert(index, uploadable);
    }

    pub fn remove_upload(&mut self, index: usize) {
        self.uploads.remove(&index);
    }

    pub fn move_upload_up(&mut self, index: usize) {
        let previous_index = self
            .uploads
            .range(..index)
            .next_back()
            .map(|(&previous_index, _)| previous_index);
        if let Some(previous_index) = previous_index {
            self.swap_uploads(previous_index, index);
        }
    }

    pub fn move_upload_down(&mut self, index: usize) {
        let next_index = self
            .uploads
            .range((Bound::Excluded(index), Bound::Unbounded))
            .next()
            .map(|(&next_index, _)| next_index);
        if let Some(next_index) = next_index {
            self.swap_uploads(index, next_index);
        }
    }

    pub fn next_pending_upload(&self) -> Option<usize> {
        self.uploads
            .iter()
            .find(|(_, uploadable)| uploadable.message.is_none())
            .map(|(&index, _)| index)
    }

    pub fn clear_upload_messages(&mut self) {
        for uploadable in self.uploads.values_mut() {
            uploadable.message = None;
        }
    }

    fn swap_uploads(&mut self, first_index: usize, second_index: usize) {
        if let Some(first) = self.uploads.remove(&first_index)
            && let Some(second) = self.uploads.insert(second_index, first)
        {
            self.uploads.insert(first_index, second);
        }
    }
}
