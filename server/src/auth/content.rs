use crate::config;
use crate::model::enums::MimeType;
use base64::engine::general_purpose::{STANDARD_NO_PAD, URL_SAFE_NO_PAD};
use base64::prelude::*;
use hmac::digest::CtOutput;
use hmac::{Mac, SimpleHmac};
use image::DynamicImage;
use once_cell::sync::Lazy;

pub fn gravatar_url(username: &str) -> String {
    let username_hash = hmac_hash(username.to_lowercase().as_bytes());
    let hex_encoded_hash = hex::encode(username_hash.into_bytes());
    format!("https://gravatar.com/avatar/{hex_encoded_hash}?d=retro&s={}", *AVATAR_WIDTH)
}

pub fn custom_avatar_url(username: &str) -> String {
    format!("{}/avatars/{}.png", *DATA_URL, username.to_lowercase())
}

pub fn post_content_url(post_id: i32, content_type: MimeType) -> String {
    format!("{}/posts/{}_{}.{}", *DATA_URL, post_id, post_security_hash(post_id), content_type.extension())
}

pub fn post_thumbnail_url(post_id: i32) -> String {
    format!("{}/generated-thumbnails/{}_{}.jpg", *DATA_URL, post_id, post_security_hash(post_id))
}

pub fn post_security_hash(post_id: i32) -> String {
    let hash = hmac_hash(&post_id.to_le_bytes());
    URL_SAFE_NO_PAD.encode(hash.into_bytes())
}

pub fn image_checksum(image: &DynamicImage) -> String {
    let hash = hmac_hash(image.as_bytes());
    STANDARD_NO_PAD.encode(hash.into_bytes())
}

type Hmac = SimpleHmac<blake3::Hasher>;

static SECRET: Lazy<&'static str> = Lazy::new(|| config::read_required_string("content_secret"));
static DATA_URL: Lazy<&'static str> = Lazy::new(|| config::read_required_string("data_url"));
static AVATAR_WIDTH: Lazy<i64> = Lazy::new(|| {
    config::read_required_table("thumbnails")
        .get("avatar_width")
        .expect("Config avatar_width should be in [thumbnails]")
        .as_integer()
        .expect("Config avatar_width should be an integer")
});

fn hmac_hash(bytes: &[u8]) -> CtOutput<Hmac> {
    let mut mac = Hmac::new_from_slice(SECRET.as_bytes()).expect("HMAC can take key of any size");
    mac.update(bytes);
    mac.finalize()
}
