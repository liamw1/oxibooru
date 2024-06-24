use crate::model::post::Post;
use crate::util;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::prelude::*;
use hmac::{Mac, SimpleHmac};
use once_cell::sync::Lazy;

pub fn post_security_hash(post_id: i32) -> String {
    let mut mac = Hmac::new_from_slice(SECRET.as_bytes()).expect("HMAC can take key of any size");
    mac.update(post_id.to_le_bytes().as_slice());
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}

pub fn post_content_url(post: &Post) -> String {
    format!("{}/posts/{}_{}.{}", *DATA_URL, post.id, post_security_hash(post.id), post.mime_type.extension())
}

pub fn post_thumbnail_url(post: &Post) -> String {
    format!("{}/generated-thumbnails/{}_{}.jpg", *DATA_URL, post.id, post_security_hash(post.id))
}

type Hmac = SimpleHmac<blake3::Hasher>;

static SECRET: Lazy<&'static str> = Lazy::new(|| util::read_required_config("content_secret"));
static DATA_URL: Lazy<&'static str> = Lazy::new(|| util::read_required_config("data_url"));
