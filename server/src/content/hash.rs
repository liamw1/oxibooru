use crate::config::Config;
use crate::filesystem::Directory;
use crate::model::enums::MimeType;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use blake3::KEY_LEN;
use diesel::deserialize::FromSql;
use diesel::expression::AsExpression;
use diesel::pg::{Pg, PgValue};
use diesel::serialize::{Output, ToSql};
use diesel::sql_types::Bytea;
use diesel::{FromSqlRow, deserialize, serialize};
use hex::{FromHex, FromHexError};
use std::fmt::Display;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Stores a `post_id` and cached post `hash`.
pub struct PostHash<'a> {
    post_id: i64,
    hash: String,
    config: &'a Config,
}

impl<'a> PostHash<'a> {
    pub fn new(config: &'a Config, post_id: i64) -> Self {
        let key: [u8; KEY_LEN] = std::array::from_fn(|i| config.content_secret.as_bytes().get(i).copied().unwrap_or(0));
        let hash = blake3::keyed_hash(&key, &post_id.to_le_bytes());
        Self {
            hash: URL_SAFE_NO_PAD.encode(hash.as_bytes()),
            post_id,
            config,
        }
    }

    pub fn id(&self) -> i64 {
        self.post_id
    }

    /// Returns URL to post content.
    pub fn content_url(&self, content_type: MimeType) -> String {
        const POSTS_DIRECTORY: Directory = Directory::Posts;
        format!("{}/{POSTS_DIRECTORY}/{self}.{}", self.config.data_url, content_type.extension())
    }

    /// Returns URL to post thumbnail. Will be a generated thumbnail by default or
    /// a custom thumbnail if it exists.
    pub fn thumbnail_url(&self) -> String {
        // Note: this requires interacting with the filesystem and might be slow
        let thumbnail_folder = if self.custom_thumbnail_path().exists() {
            Directory::CustomThumbnails
        } else {
            Directory::GeneratedThumbnails
        };
        format!("{}/{thumbnail_folder}/{self}.{THUMBNAIL_EXTENSION}", self.config.data_url)
    }

    /// Returns path to post content on disk.
    pub fn content_path(&self, content_type: MimeType) -> PathBuf {
        let filename = format!("{self}.{}", content_type.extension());
        self.config.path(Directory::Posts).join(filename)
    }

    /// Returns path to generated post thumbnail on disk.
    pub fn generated_thumbnail_path(&self) -> PathBuf {
        let filename = format!("{self}.{THUMBNAIL_EXTENSION}");
        self.config.path(Directory::GeneratedThumbnails).join(filename)
    }

    /// Returns path to custom post thumbnail on disk.
    pub fn custom_thumbnail_path(&self) -> PathBuf {
        let filename = format!("{self}.{THUMBNAIL_EXTENSION}");
        self.config.path(Directory::CustomThumbnails).join(filename)
    }
}

impl Display for PostHash<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let outer_bucket = self.post_id / 1_000_000;
        let inner_bucket = (self.post_id / 10_000) % 100;
        write!(f, "{outer_bucket:06}/{inner_bucket:02}/{}_{}", self.post_id, self.hash)
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
        <[u8] as ToSql<Bytea, Pg>>::to_sql(self.as_ref(), out)
    }
}

impl<const N: usize> FromSql<Bytea, Pg> for GenericChecksum<N> {
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        Ok(Self::from_bytes(value.as_bytes()))
    }
}

pub fn gravatar_url(config: &Config, username: &str) -> String {
    let username_hash = blake3::hash(username.to_lowercase().as_bytes());
    let hex_encoded_hash = hex::encode(username_hash.as_bytes());
    format!("https://gravatar.com/avatar/{hex_encoded_hash}?d=retro&s={}", config.thumbnails.avatar_width)
}

/// Computes the BLAKE3 and MD5 checksums of the file at `path` in a single pass.
///
/// BLAKE3 is strongly preferred for duplicate detection. MD5 is vulnerable to collisions
/// and is only computed for convience for search on other sites.
pub fn compute_checksums(path: &Path) -> std::io::Result<(Checksum, Md5Checksum)> {
    const KB: usize = 1024;
    const BUFFER_CAPACITY: usize = 64 * KB;

    let mut file = File::open(path)?;
    let mut md5_ctx = md5::Context::new();
    let mut blake3_hasher = blake3::Hasher::new();

    let mut buffer = [0; BUFFER_CAPACITY];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }

        blake3_hasher.update(&buffer[..n]);
        md5_ctx.consume(&buffer[..n]);
    }

    let checksum = GenericChecksum(blake3_hasher.finalize().into());
    let md5_checksum = GenericChecksum(md5_ctx.compute().0);
    Ok((checksum, md5_checksum))
}

/// Similar to [`compute_checksum`], except checksum is base64 encoded.
pub fn compute_url_safe_hash(content: &[u8]) -> String {
    let hash = blake3::hash(content);
    URL_SAFE_NO_PAD.encode(hash.as_bytes())
}

const THUMBNAIL_EXTENSION: &str = "jpg";
