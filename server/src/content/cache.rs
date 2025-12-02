use crate::api::ApiResult;
use crate::app::AppState;
use crate::content::hash::{Checksum, Md5Checksum};
use crate::content::signature::COMPRESSED_SIGNATURE_LEN;
use crate::content::thumbnail::ThumbnailType;
use crate::content::{FileContents, decode, hash, signature, thumbnail};
use crate::filesystem::{self, Directory};
use crate::model::enums::{MimeType, PostFlag, PostFlags, PostType};
use image::DynamicImage;
use image::error::LimitErrorKind;
use std::collections::VecDeque;

/// Stores properties of content that are costly to compute (usually require reading/decoding entire file).
#[derive(Clone)]
pub struct CachedProperties {
    pub token: String,
    pub checksum: Checksum,
    pub md5_checksum: Md5Checksum,
    pub signature: [i64; COMPRESSED_SIGNATURE_LEN],
    pub thumbnail: DynamicImage,
    pub width: i32,
    pub height: i32,
    pub mime_type: MimeType,
    pub file_size: i64,
    pub flags: PostFlags,
}

/// A simple ring buffer that stores [`CachedProperties`].
pub struct RingCache {
    data: VecDeque<(String, CachedProperties)>,
    max_size: usize,
}

impl RingCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            data: VecDeque::new(),
            max_size,
        }
    }

    pub fn clear(&mut self) {
        self.data = VecDeque::new();
    }

    fn insert(&mut self, key: String, value: CachedProperties) {
        self.data.push_back((key, value));
        if self.data.len() > self.max_size {
            self.data.pop_front();
        }
    }

    fn remove(&mut self, key: &str) -> Option<CachedProperties> {
        self.data
            .iter()
            .position(|(cache_key, _)| cache_key == key)
            .and_then(|pos| self.data.remove(pos))
            .map(|(_, cache_value)| cache_value)
    }
}

/// Computes content properties and caches them in memory.
pub fn compute_properties(state: &AppState, content_token: String) -> ApiResult<CachedProperties> {
    let properties = compute_properties_no_cache(state, content_token.clone())?;

    // Clone this here to make sure we aren't holding onto lock for longer than necessary
    let properties_copy = properties.clone();
    state.get_content_cache().insert(content_token, properties_copy);

    Ok(properties)
}

/// Returns cached properties of content or computes them if not in cache.
pub fn get_or_compute_properties(state: &AppState, content_token: String) -> ApiResult<CachedProperties> {
    let maybe_properties = state.get_content_cache().remove(&content_token);
    match maybe_properties {
        Some(properties) => Ok(properties),
        None => compute_properties_no_cache(state, content_token),
    }
}

/// Computes content properties without storing them in cache.
fn compute_properties_no_cache(state: &AppState, token: String) -> ApiResult<CachedProperties> {
    let temp_path = state.config.path(Directory::TemporaryUploads).join(&token);
    let file_size = filesystem::file_size(&temp_path)?;
    let data = std::fs::read(&temp_path)?;
    let checksum = hash::compute_checksum(&data);
    let md5_checksum = hash::compute_md5_checksum(&data);

    let (_uuid, extension) = token.split_once('.').unwrap_or((&token, ""));
    let mime_type = MimeType::from_extension(extension)?;
    let post_type = PostType::from(mime_type);

    let has_sound = match post_type {
        PostType::Image | PostType::Animation => false,
        PostType::Video => decode::video_has_audio(&temp_path)?,
        PostType::Flash => decode::swf_has_audio(&temp_path)?,
    };
    let flags = if has_sound {
        PostFlags::new_with(PostFlag::Sound)
    } else {
        PostFlags::new()
    };

    let file_contents = FileContents { data, mime_type };
    let image = decode::representative_image(&state.config, &file_contents, &temp_path)?;

    Ok(CachedProperties {
        token,
        checksum,
        md5_checksum,
        signature: signature::compute(&image),
        thumbnail: thumbnail::create(&state.config, &image, ThumbnailType::Post),
        width: i32::try_from(image.width()).map_err(|_| LimitErrorKind::DimensionError)?,
        height: i32::try_from(image.height()).map_err(|_| LimitErrorKind::DimensionError)?,
        mime_type,
        file_size,
        flags,
    })
}
