use crate::filesystem::Directory;
use crate::model::enums::MimeType;
use crate::{config, filesystem};
use base64::engine::general_purpose::{STANDARD_NO_PAD, URL_SAFE_NO_PAD};
use base64::prelude::*;
use hmac::digest::CtOutput;
use hmac::{Mac, SimpleHmac};
use std::path::PathBuf;

pub struct PostHash {
    hash: String,
    post_id: i32,
}

impl PostHash {
    pub fn new(post_id: i32) -> Self {
        Self {
            hash: URL_SAFE_NO_PAD.encode(hmac_hash(&post_id.to_le_bytes()).into_bytes()),
            post_id,
        }
    }

    pub fn content_url(&self, content_type: MimeType) -> String {
        format!("{}/posts/{}_{}.{}", config::get().data_url, self.post_id, self.hash, content_type.extension())
    }

    pub fn thumbnail_url(&self) -> String {
        let thumbnail_folder = match self.custom_thumbnail_path().exists() {
            true => "custom-thumbnails",
            false => "generated-thumbnails",
        };
        format!("{}/{thumbnail_folder}/{}_{}.jpg", config::get().data_url, self.post_id, self.hash)
    }

    pub fn content_path(&self, content_type: MimeType) -> PathBuf {
        format!(
            "{}/{}_{}.{}",
            filesystem::as_str(Directory::Posts),
            self.post_id,
            self.hash,
            content_type.extension()
        )
        .into()
    }

    pub fn generated_thumbnail_path(&self) -> PathBuf {
        format!("{}/{}_{}.jpg", filesystem::as_str(Directory::GeneratedThumbnails), self.post_id, self.hash).into()
    }

    pub fn custom_thumbnail_path(&self) -> PathBuf {
        format!("{}/{}_{}.jpg", filesystem::as_str(Directory::CustomThumbnails), self.post_id, self.hash).into()
    }
}

pub fn gravatar_url(username: &str) -> String {
    let username_hash = hmac_hash(username.to_lowercase().as_bytes());
    let hex_encoded_hash = hex::encode(username_hash.into_bytes());
    format!("https://gravatar.com/avatar/{hex_encoded_hash}?d=retro&s={}", config::get().thumbnails.avatar_width)
}

pub fn custom_avatar_url(username: &str) -> String {
    format!("{}/custom-avatars/{}.png", config::get().data_url, username.to_lowercase())
}

pub fn custom_avatar_path(username: &str) -> PathBuf {
    format!("{}/{}.png", filesystem::as_str(Directory::CustomAvatars), username.to_lowercase()).into()
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

pub fn compute_md5_checksum(content: &[u8]) -> String {
    let digest = md5::compute(content);
    String::from_utf8_lossy(&digest.0).into_owned()
}

pub fn compute_url_safe_hash(content: &str) -> String {
    let hash = hmac_hash(content.as_bytes());
    URL_SAFE_NO_PAD.encode(hash.into_bytes())
}

type Hmac = SimpleHmac<blake3::Hasher>;

fn hmac_hash(bytes: &[u8]) -> CtOutput<Hmac> {
    let mut mac = Hmac::new_from_slice(config::get().content_secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(bytes);
    mac.finalize()
}
