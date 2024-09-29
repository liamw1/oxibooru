use crate::api::ApiResult;
use crate::auth::content;
use crate::content::{decode, signature};
use crate::model::enums::{MimeType, PostFlag, PostFlags, PostType};
use crate::{config, filesystem};
use image::DynamicImage;
use std::collections::VecDeque;
use std::sync::{LazyLock, Mutex, MutexGuard};

#[derive(Clone)]
pub struct CachedProperties {
    pub checksum: String,
    pub signature: Vec<u8>,
    pub thumbnail: DynamicImage,
    pub width: u32,
    pub height: u32,
    pub mime_type: MimeType,
    pub file_size: u64,
    pub flags: PostFlags,
}

pub fn compute_properties(content_token: &str) -> ApiResult<CachedProperties> {
    let temp_path = filesystem::temporary_upload_filepath(content_token);
    let file_size = std::fs::metadata(&temp_path)?.len();
    let file_contents = std::fs::read(&temp_path)?;
    let checksum = content::compute_checksum(&file_contents);

    let (_uuid, extension) = content_token.split_once('.').unwrap();
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

    let image = match PostType::from(mime_type) {
        PostType::Image | PostType::Animation => {
            let image_format = mime_type
                .to_image_format()
                .expect("Mime type should be convertable to image format");
            decode::image(&file_contents, image_format)?
        }
        PostType::Video => decode::video_frame(&temp_path)?,
    };
    let signature = signature::compute_signature(&image);

    let thumbnail = image.resize_to_fill(
        config::get().thumbnails.post_width,
        config::get().thumbnails.post_height,
        image::imageops::FilterType::Gaussian,
    );

    let properties = CachedProperties {
        checksum,
        signature,
        thumbnail,
        width: image.width(),
        height: image.height(),
        mime_type,
        file_size,
        flags,
    };

    // Clone these here to make sure we aren't holding onto lock for longer than necessary
    let content_token_copy = content_token.to_owned();
    let properties_copy = properties.clone();
    get_cache_guard().insert(content_token_copy, properties_copy);

    Ok(properties)
}

pub fn get_or_compute_properties(content_token: &str) -> ApiResult<CachedProperties> {
    let maybe_properties = get_cache_guard().remove(content_token);
    match maybe_properties {
        Some(properties) => Ok(properties),
        None => compute_properties(content_token),
    }
}

static CONTENT_CACHE: LazyLock<Mutex<RingCache>> = LazyLock::new(|| Mutex::new(RingCache::new(10)));

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
