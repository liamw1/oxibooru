use crate::filesystem::Directory;
use crate::model::enums::MimeType;
use crate::{config, filesystem};
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
use std::path::PathBuf;
use std::str::FromStr;

/// Stores a `post_id` and post `hash`.
pub struct PostHash {
    post_id: i64,
    hash: String,
}

impl PostHash {
    pub fn new(post_id: i64) -> Self {
        Self {
            hash: URL_SAFE_NO_PAD.encode(hmac_hash(&post_id.to_le_bytes()).into_bytes()),
            post_id,
        }
    }

    pub fn id(&self) -> i64 {
        self.post_id
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

pub type Checksum = GenericChecksum<32>;
pub type Md5Checksum = GenericChecksum<16>;

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

pub fn gravatar_url(username: &str) -> String {
    let username_hash = hmac_hash(username.to_lowercase().as_bytes());
    let hex_encoded_hash = hex::encode(username_hash.into_bytes());
    format!("https://gravatar.com/avatar/{hex_encoded_hash}?d=retro&s={}", config::get().thumbnails.avatar_width)
}

pub fn custom_avatar_url(username: &str) -> String {
    format!("{}/avatars/{}.png", config::get().data_url, username.to_lowercase())
}

pub fn custom_avatar_path(username: &str) -> PathBuf {
    format!("{}/{}.png", filesystem::as_str(Directory::Avatars), username.to_lowercase()).into()
}

/// Computes a checksum for duplicate detection. Uses raw file data instead of decoded
/// pixel data because different compression schemes can compress identical pixel data
/// in different ways.
pub fn compute_checksum(content: &[u8]) -> Checksum {
    let hash = hmac_hash(content);
    GenericChecksum(hash.into_bytes().into())
}

pub fn compute_md5_checksum(content: &[u8]) -> Md5Checksum {
    let digest = md5::compute(content);
    GenericChecksum(digest.0)
}

pub fn compute_url_safe_hash(content: &str) -> String {
    let hash = hmac_hash(content.as_bytes());
    URL_SAFE_NO_PAD.encode(hash.into_bytes())
}

type Hmac = SimpleHmac<blake3::Hasher>;

fn hmac_hash(bytes: &[u8]) -> CtOutput<Hmac> {
    let mut mac =
        Hmac::new_from_slice(config::get().content_secret.as_bytes()).expect("HMAC should take key of any size");
    mac.update(bytes);
    mac.finalize()
}
