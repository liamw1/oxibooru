use crate::config;
use crate::model::enums::MimeType;
use base64::engine::general_purpose::{STANDARD_NO_PAD, URL_SAFE_NO_PAD};
use base64::prelude::*;
use hmac::digest::CtOutput;
use hmac::{Mac, SimpleHmac};
use std::path::PathBuf;

pub fn gravatar_url(username: &str) -> String {
    let username_hash = hmac_hash(username.to_lowercase().as_bytes());
    let hex_encoded_hash = hex::encode(username_hash.into_bytes());
    format!("https://gravatar.com/avatar/{hex_encoded_hash}?d=retro&s={}", config::get().thumbnails.avatar_width)
}

pub fn custom_avatar_url(username: &str) -> String {
    format!("{}/avatars/{}.png", config::get().data_url, username.to_lowercase())
}

// TODO: Create wrapper class over hash that can compute file paths

// NOTE: These could be tied together to avoid computing hash twice
pub fn post_content_url(post_id: i32, content_type: MimeType) -> String {
    format!(
        "{}/posts/{post_id}_{}.{}",
        config::get().data_url,
        post_security_hash(post_id),
        content_type.extension()
    )
}
pub fn post_thumbnail_url(post_id: i32) -> String {
    let hash = post_security_hash(post_id);
    match custom_thumbnail_path(post_id).exists() {
        true => format!("{}/custom-thumbnails/{post_id}_{}.jpg", config::get().data_url, hash),
        false => format!("{}/generated-thumbnails/{post_id}_{}.jpg", config::get().data_url, hash),
    }
}

// NOTE: These could be tied together to avoid computing hash twice
pub fn post_content_path(post_id: i32, content_type: MimeType) -> PathBuf {
    format!(
        "{}/posts/{post_id}_{}.{}",
        config::data_dir(),
        post_security_hash(post_id),
        content_type.extension()
    )
    .into()
}
pub fn generated_thumbnail_path(post_id: i32) -> PathBuf {
    format!("{}/generated-thumbnails/{post_id}_{}.jpg", config::data_dir(), post_security_hash(post_id)).into()
}
pub fn custom_thumbnail_path(post_id: i32) -> PathBuf {
    format!("{}/custom-thumbnails/{post_id}_{}.jpg", config::data_dir(), post_security_hash(post_id)).into()
}

/*
    Computes a checksum for duplicate detection. Uses raw file data instead of decoded
    pixel data because different compression schemes can compress identical pixel data
    in different ways.
*/
pub fn compute_checksum(content: &[u8]) -> String {
    let hash = hmac_hash(content);
    STANDARD_NO_PAD.encode(hash.into_bytes())
}

type Hmac = SimpleHmac<blake3::Hasher>;

fn post_security_hash(post_id: i32) -> String {
    let hash = hmac_hash(&post_id.to_le_bytes());
    URL_SAFE_NO_PAD.encode(hash.into_bytes())
}

fn hmac_hash(bytes: &[u8]) -> CtOutput<Hmac> {
    let mut mac = Hmac::new_from_slice(config::get().content_secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(bytes);
    mac.finalize()
}
