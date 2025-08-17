use crate::api::ApiResult;
use crate::content::signature::COMPRESSED_SIGNATURE_LEN;
use crate::content::thumbnail::ThumbnailType;
use crate::content::{FileContents, decode, hash, signature, thumbnail};
use crate::filesystem;
use crate::model::enums::{MimeType, PostFlag, PostFlags, PostType};
use image::DynamicImage;
use std::collections::VecDeque;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use tracing::error;

/// Stores properties of content that are costly to compute (usually requires reading/decoding entire file).
pub struct CachedProperties {
    pub token: String,
    pub checksum: Vec<u8>,
    pub md5_checksum: [u8; 16],
    pub signature: [i64; COMPRESSED_SIGNATURE_LEN],
    pub thumbnail: DynamicImage,
    pub width: i32,
    pub height: i32,
    pub mime_type: MimeType,
    pub file_size: i64,
    pub flags: PostFlags,
}

/// Computes content properties and caches them in memory.
pub fn compute_properties(content_token: String) -> ApiResult<Arc<CachedProperties>> {
    let properties = compute_properties_no_cache(content_token.clone())?;
    get_cache_guard().insert(content_token, properties.clone());
    Ok(properties)
}

/// Returns cached properties of content or computes them if not in cache.
pub fn get_or_compute_properties(content_token: String) -> ApiResult<Arc<CachedProperties>> {
    let maybe_properties = get_cache_guard().remove(&content_token);
    match maybe_properties {
        Some(properties) => Ok(properties),
        None => compute_properties_no_cache(content_token),
    }
}

/// A simple ring buffer that stores [`CachedProperties`].
struct RingCache {
    data: VecDeque<(String, Arc<CachedProperties>)>,
    capacity: usize,
}

impl RingCache {
    /// Constructs a new cache with given `capacity`.
    fn new(capacity: usize) -> Self {
        Self {
            data: VecDeque::new(),
            capacity,
        }
    }

    /// Inserts `key`-`value` pair into the cache.
    /// If the number of elements in the cache is equal to the `capacity`,
    /// the least-recently inserted element will be evicted.
    fn insert(&mut self, key: String, value: Arc<CachedProperties>) {
        self.data.push_back((key, value));
        if self.data.len() > self.capacity {
            self.data.pop_front();
        }
    }

    /// Removes and returns the element with the given `key`.
    fn remove(&mut self, key: &str) -> Option<Arc<CachedProperties>> {
        self.data
            .iter()
            .position(|(cache_key, _)| cache_key == key)
            .and_then(|pos| self.data.remove(pos))
            .map(|(_, cache_value)| cache_value)
    }

    /// Destroys all elements currently in the cache.
    fn reset(&mut self) {
        self.data = VecDeque::new();
    }
}

/// Returns a [`MutexGuard`] to content properties cache.
fn get_cache_guard() -> MutexGuard<'static, RingCache> {
    /// Max number of elements in the content cache. Should be as large as the number of users expected to be uploading concurrently.
    const CONTENT_CACHE_SIZE: usize = 10;
    static CONTENT_CACHE: LazyLock<Mutex<RingCache>> = LazyLock::new(|| Mutex::new(RingCache::new(CONTENT_CACHE_SIZE)));

    match CONTENT_CACHE.lock() {
        Ok(guard) => guard,
        Err(err) => {
            error!("Content cache has been poisoned! Resetting...");
            let mut guard = err.into_inner();
            guard.reset();
            guard
        }
    }
}

/// Computes content properties without storing them in cache.
fn compute_properties_no_cache(token: String) -> ApiResult<Arc<CachedProperties>> {
    let temp_path = filesystem::temporary_upload_filepath(&token);
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
    let flags = match has_sound {
        true => PostFlags::new_with(PostFlag::Sound),
        false => PostFlags::new(),
    };

    let file_contents = FileContents { data, mime_type };
    let image = decode::representative_image(&file_contents, &temp_path)?;

    Ok(Arc::new(CachedProperties {
        token,
        checksum,
        md5_checksum,
        signature: signature::compute(&image),
        thumbnail: thumbnail::create(&image, ThumbnailType::Post),
        width: i32::try_from(image.width()).unwrap(),
        height: i32::try_from(image.height()).unwrap(),
        mime_type,
        file_size,
        flags,
    }))
}
