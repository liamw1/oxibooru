use crate::api::ApiResult;
use crate::content::signature::COMPRESSED_SIGNATURE_SIZE;
use crate::content::{decode, hash, signature, thumbnail};
use crate::filesystem;
use crate::model::enums::{MimeType, PostFlag, PostFlags, PostType};
use image::DynamicImage;
use std::collections::VecDeque;
use std::sync::{LazyLock, Mutex, MutexGuard};

#[derive(Clone)]
pub struct CachedProperties {
    pub token: String,
    pub checksum: String,
    pub md5_checksum: String,
    pub signature: [i64; COMPRESSED_SIGNATURE_SIZE],
    pub thumbnail: DynamicImage,
    pub width: u32,
    pub height: u32,
    pub mime_type: MimeType,
    pub file_size: u64,
    pub flags: PostFlags,
}

pub fn compute_properties(content_token: String) -> ApiResult<CachedProperties> {
    let properties = compute_properties_no_cache(content_token.clone())?;

    // Clone this here to make sure we aren't holding onto lock for longer than necessary
    let properties_copy = properties.clone();
    get_cache_guard().insert(content_token, properties_copy);

    Ok(properties)
}

pub fn get_or_compute_properties(content_token: String) -> ApiResult<CachedProperties> {
    let maybe_properties = get_cache_guard().remove(&content_token);
    match maybe_properties {
        Some(properties) => Ok(properties),
        None => compute_properties_no_cache(content_token),
    }
}

// Max number of elements in the content cache. Should be as large as the number of users expected to be uploading concurrently.
const CONTENT_CACHE_SIZE: usize = 10;
static CONTENT_CACHE: LazyLock<Mutex<RingCache>> = LazyLock::new(|| Mutex::new(RingCache::new(CONTENT_CACHE_SIZE)));

struct RingCache {
    data: VecDeque<(String, CachedProperties)>,
    max_size: usize,
}

impl RingCache {
    fn new(max_size: usize) -> Self {
        Self {
            data: VecDeque::new(),
            max_size,
        }
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
            .position(|entry| entry.0 == key)
            .and_then(|pos| self.data.remove(pos))
            .map(|entry| entry.1)
    }

    fn reset(&mut self) {
        self.data = VecDeque::new()
    }
}

fn get_cache_guard() -> MutexGuard<'static, RingCache> {
    match CONTENT_CACHE.lock() {
        Ok(guard) => guard,
        Err(err) => {
            eprintln!("Content cache has been poisoned! Resetting...");
            let mut guard = err.into_inner();
            guard.reset();
            guard
        }
    }
}

fn compute_properties_no_cache(token: String) -> ApiResult<CachedProperties> {
    let temp_path = filesystem::temporary_upload_filepath(&token);
    let file_size = std::fs::metadata(&temp_path)?.len();
    let file_contents = std::fs::read(&temp_path)?;
    let checksum = hash::compute_checksum(&file_contents);
    let md5_checksum = hash::compute_md5_checksum(&file_contents);

    let (_uuid, extension) = token.split_once('.').unwrap();
    let mime_type = MimeType::from_extension(extension)?;
    let post_type = PostType::from(mime_type);

    let flags = match post_type {
        PostType::Image | PostType::Animation => PostFlags::new(),
        PostType::Video => {
            if decode::has_audio(&temp_path)? {
                PostFlags::new_with(PostFlag::Sound)
            } else {
                PostFlags::new()
            }
        }
    };
    let image = decode::representative_image(&file_contents, &temp_path, mime_type)?;

    Ok(CachedProperties {
        token,
        checksum,
        md5_checksum,
        signature: signature::compute(&image),
        thumbnail: thumbnail::create(&image),
        width: image.width(),
        height: image.height(),
        mime_type,
        file_size,
        flags,
    })
}
