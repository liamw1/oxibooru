use crate::config::Config;
use crate::filesystem::Directory;
use crate::model::enums::MimeType;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use diesel::deserialize::FromSql;
use diesel::expression::AsExpression;
use diesel::pg::{Pg, PgValue};
use diesel::serialize::{Output, ToSql};
use diesel::sql_types::Bytea;
use diesel::{FromSqlRow, deserialize, serialize};
use hex::{FromHex, FromHexError};
use hmac::digest::CtOutput;
use hmac::{Mac, SimpleHmac};
use std::fmt::Display;
use std::path::PathBuf;
use std::str::FromStr;

/// Stores a `post_id` and cached post `hash`.
pub struct PostHash<'a> {
    post_id: i64,
    hash: String,
    config: &'a Config,
}

impl<'a> PostHash<'a> {
    pub fn new(config: &'a Config, post_id: i64) -> Self {
        Self {
            hash: compute_url_safe_hash(config, &post_id.to_le_bytes()),
            post_id,
            config,
        }
    }

    pub fn id(&self) -> i64 {
        self.post_id
    }

    /// Returns URL to post content.
    pub fn content_url(&self, content_type: MimeType) -> String {
        format!("{}/posts/{self}.{}", self.config.data_url, content_type.extension())
    }

    /// Returns URL to post thumbnail. Will be a generated thumbnail by default or
    /// a custom thumbnail if it exists.
    pub fn thumbnail_url(&self) -> String {
        // Note: this requires interacting with the filesystem and might be slow
        let thumbnail_folder = if self.custom_thumbnail_path().exists() {
            "custom-thumbnails"
        } else {
            "generated-thumbnails"
        };
        format!("{}/{thumbnail_folder}/{self}.jpg", self.config.data_url)
    }

    /// Returns path to post content on disk.
    pub fn content_path(&self, content_type: MimeType) -> PathBuf {
        let filename = format!("{self}.{}", content_type.extension());
        self.config.path(Directory::Posts).join(filename)
    }

    /// Returns path to generated post thumbnail on disk.
    pub fn generated_thumbnail_path(&self) -> PathBuf {
        let filename = format!("{self}.jpg");
        self.config.path(Directory::GeneratedThumbnails).join(filename)
    }

    /// Returns path to custom post thumbnail on disk.
    pub fn custom_thumbnail_path(&self) -> PathBuf {
        let filename = format!("{self}.jpg");
        self.config.path(Directory::CustomThumbnails).join(filename)
    }
}

impl Display for PostHash<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}_{}", self.post_id, self.hash)
    }
}

pub type Checksum = GenericChecksum<32>;
pub type Md5Checksum = GenericChecksum<16>;

/// Represents a fixed-size checksum of length `N`.
/// Can be deserialized from the database without allocation.
#[derive(Debug, Clone, PartialEq, Eq, AsExpression, FromSqlRow)]
#[diesel(sql_type = Bytea)]
pub struct GenericChecksum<const N: usize>([u8; N]);

impl<const N: usize> GenericChecksum<N> {
    /// Constructs a [`GenericChecksum`] using the first `N` values in a slice of `bytes`.
    /// If the length of the slice is less than `N`, the remaining checksum bytes will be set to 0.
    pub const fn from_bytes(bytes: &[u8]) -> Self {
        let mut checksum = [0; N];
        let mut index = 0;
        while index < bytes.len() && index < N {
            checksum[index] = bytes[index];
            index += 1;
        }
        Self(checksum)
    }
}

impl<const N: usize> AsRef<[u8]> for GenericChecksum<N> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl<const N: usize> From<[u8; N]> for GenericChecksum<N> {
    fn from(value: [u8; N]) -> Self {
        Self(value)
    }
}

impl<const N: usize> FromStr for GenericChecksum<N>
where
    [u8; N]: FromHex<Error = FromHexError>,
{
    type Err = FromHexError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        <[u8; N]>::from_hex(s).map(Self)
    }
}

impl<const N: usize> ToSql<Bytea, Pg> for GenericChecksum<N> {
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Pg>) -> serialize::Result {
        <[u8] as ToSql<Bytea, Pg>>::to_sql(self.0.as_slice(), out)
    }
}

impl<const N: usize> FromSql<Bytea, Pg> for GenericChecksum<N> {
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        Ok(Self::from_bytes(value.as_bytes()))
    }
}

pub fn gravatar_url(config: &Config, username: &str) -> String {
    let username_hash = hmac_hash(config, username.to_lowercase().as_bytes());
    let hex_encoded_hash = hex::encode(username_hash.into_bytes());
    format!("https://gravatar.com/avatar/{hex_encoded_hash}?d=retro&s={}", config.thumbnails.avatar_width)
}

/// Computes a checksum for duplicate detection. Uses raw file data instead of decoded
/// pixel data because different compression schemes can compress identical pixel data
/// in different ways.
pub fn compute_checksum(config: &Config, content: &[u8]) -> Checksum {
    let hash = hmac_hash(config, content);
    GenericChecksum(hash.into_bytes().into())
}

/// Computes MD5 checksum. Not used for duplicate detection due to its vulnerability
/// to collisions.
pub fn compute_md5_checksum(content: &[u8]) -> Md5Checksum {
    let digest = md5::compute(content);
    GenericChecksum(digest.0)
}

/// Similar to [`compute_checksum`], except checksum is base64 encoded.
pub fn compute_url_safe_hash(config: &Config, content: &[u8]) -> String {
    let hash = hmac_hash(config, content);
    URL_SAFE_NO_PAD.encode(hash.into_bytes())
}

type Hmac = SimpleHmac<blake3::Hasher>;

fn hmac_hash(config: &Config, bytes: &[u8]) -> CtOutput<Hmac> {
    let mut mac = Hmac::new_from_slice(config.content_secret.as_bytes()).expect("HMAC should take key of any size");
    mac.update(bytes);
    mac.finalize()
}
