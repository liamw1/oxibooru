use crate::config;
use crate::model::post::Post;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::prelude::*;
use hmac::{Mac, SimpleHmac};
use once_cell::sync::Lazy;

pub fn gravatar_url(username: &str) -> String {
    let username_hash = url_safe_hash(username.to_lowercase().as_bytes());
    format!("https://gravatar.com/avatar/{}?d=retro&s={}", username_hash, *AVATAR_WIDTH)
}

pub fn manual_url(username: &str) -> String {
    format!("{}/avatars/{}.png", *DATA_URL, username.to_lowercase())
}

pub fn post_content_url(post: &Post) -> String {
    format!("{}/posts/{}_{}.{}", *DATA_URL, post.id, post_security_hash(post.id), post.mime_type.extension())
}

pub fn post_thumbnail_url(post: &Post) -> String {
    format!("{}/generated-thumbnails/{}_{}.jpg", *DATA_URL, post.id, post_security_hash(post.id))
}

type Hmac = SimpleHmac<blake3::Hasher>;

static SECRET: Lazy<&'static str> = Lazy::new(|| config::read_required_string("content_secret"));
static DATA_URL: Lazy<&'static str> = Lazy::new(|| config::read_required_string("data_url"));
static AVATAR_WIDTH: Lazy<i64> = Lazy::new(|| {
    config::read_required_table("thumbnails")
        .get("avatar_width")
        .unwrap_or_else(|| panic!("Config avatar_width missing from [thumbnails]"))
        .as_integer()
        .unwrap_or_else(|| panic!("Config avatar_width is not an integer"))
});

fn url_safe_hash(bytes: &[u8]) -> String {
    let mut mac = Hmac::new_from_slice(SECRET.as_bytes()).expect("HMAC can take key of any size");
    mac.update(bytes);
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}

fn post_security_hash(post_id: i32) -> String {
    url_safe_hash(&post_id.to_le_bytes())
}
