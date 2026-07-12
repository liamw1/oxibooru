use crate::api::error::ApiResult;
use crate::content::hash::{Checksum, Md5Checksum};
use crate::content::signature::COMPRESSED_SIGNATURE_LEN;
use crate::content::thumbnail::ThumbnailType;
use crate::content::upload::UploadToken;
use crate::content::{decode, hash, signature, thumbnail};
use crate::extract::Ctx;
use crate::model::enums::{MimeType, PostFlag, PostFlags, PostType};
use crate::{content, filesystem};
use image::DynamicImage;
use image::error::LimitErrorKind;
use std::collections::VecDeque;

/// Stores properties of content that are costly to compute (usually require reading/decoding entire file).
#[derive(Clone)]
pub struct CachedProperties {
    pub token: UploadToken,
    pub checksum: Checksum,
    pub md5_checksum: Md5Checksum,
    pub signature: [i64; COMPRESSED_SIGNATURE_LEN],
    pub thumbnail: DynamicImage,
    pub width: i32,
    pub height: i32,
    pub mime_type: MimeType,
    pub post_type: PostType,
    pub file_size: i64,
    pub flags: PostFlags,
}

/// A simple ring buffer that stores [`CachedProperties`].
pub struct RingCache {
    data: VecDeque<CachedProperties>,
    max_size: usize,
}

impl RingCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            data: VecDeque::new(),
            max_size,
        }
    }

    fn insert(&mut self, value: CachedProperties) {
        self.data.push_back(value);
        if self.data.len() > self.max_size {
            self.data.pop_front();
        }
    }

    fn remove(&mut self, key: &UploadToken) -> Option<CachedProperties> {
        self.data
            .iter()
            .position(|value| &value.token == key)
            .and_then(|pos| self.data.remove(pos))
    }
}

/// Computes content properties and caches them in memory.
pub fn compute_properties(ctx: &Ctx, content_token: UploadToken) -> ApiResult<CachedProperties> {
    let properties = compute_properties_no_cache(ctx, content_token)?;

    let properties_copy = properties.clone();
    ctx.get_content_cache().insert(properties);

    Ok(properties_copy)
}

/// Returns cached properties of content or computes them if not in cache.
pub fn remove_or_compute_properties(ctx: &Ctx, content_token: UploadToken) -> ApiResult<CachedProperties> {
    let maybe_properties = ctx.get_content_cache().remove(&content_token);
    match maybe_properties {
        Some(properties) => Ok(properties),
        None => compute_properties_no_cache(ctx, content_token),
    }
}

/// Computes content properties without storing them in cache.
fn compute_properties_no_cache(ctx: &Ctx, token: UploadToken) -> ApiResult<CachedProperties> {
    let temp_path = token.path(&ctx.config);
    let file_size = content::map_read_result(filesystem::file_size(&temp_path))?;
    let (checksum, md5_checksum) = content::map_read_result(hash::compute_checksums(&temp_path))?;

    let mime_type = token.mime_type();
    let post_type = decode::detect_post_type(&temp_path, mime_type)?;
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

    let image = decode::representative_image(&ctx.config, &temp_path, mime_type)?;

    Ok(CachedProperties {
        token,
        checksum,
        md5_checksum,
        signature: signature::compute(&image),
        thumbnail: thumbnail::create(&ctx.config, &image, ThumbnailType::Post),
        width: i32::try_from(image.width()).map_err(|_| LimitErrorKind::DimensionError)?,
        height: i32::try_from(image.height()).map_err(|_| LimitErrorKind::DimensionError)?,
        mime_type,
        post_type,
        file_size,
        flags,
    })
}
