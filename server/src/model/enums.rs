use crate::auth;
use diesel::deserialize::{self, FromSql};
use diesel::pg::Pg;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::SmallInt;
use diesel::AsExpression;
use diesel::FromSqlRow;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use thiserror::Error;

#[derive(Debug, Copy, Clone, PartialEq, Eq, FromPrimitive, AsExpression, FromSqlRow, Serialize, Deserialize)]
#[diesel(sql_type = SmallInt)]
#[serde(rename_all = "lowercase")]
pub enum AvatarStyle {
    Gravatar,
    Manual,
}

impl ToSql<SmallInt, Pg> for AvatarStyle
where
    i16: ToSql<SmallInt, Pg>,
{
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        let value = *self as i16;
        <i16 as ToSql<SmallInt, Pg>>::to_sql(&value, &mut out.reborrow())
    }
}

impl FromSql<SmallInt, Pg> for AvatarStyle
where
    i16: FromSql<SmallInt, Pg>,
{
    fn from_sql(bytes: <Pg as diesel::backend::Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let database_value = i16::from_sql(bytes)?;
        AvatarStyle::from_i16(database_value).ok_or(DeserializeAvatarStyleError.into())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, FromPrimitive, AsExpression, FromSqlRow, Serialize, Deserialize)]
#[diesel(sql_type = SmallInt)]
#[serde(rename_all = "lowercase")]
pub enum PostType {
    Image,
    Animation,
    Video,
    Flash,
    Youtube,
}

impl From<MimeType> for PostType {
    fn from(value: MimeType) -> Self {
        match value {
            MimeType::BMP => Self::Image,
            MimeType::GIF => Self::Image,
            MimeType::JPEG => Self::Image,
            MimeType::PNG => Self::Image,
            MimeType::WEBP => Self::Image,
            MimeType::MP4 => Self::Video,
            MimeType::MOV => Self::Video,
            MimeType::WEBM => Self::Video,
        }
    }
}

impl ToSql<SmallInt, Pg> for PostType
where
    i16: ToSql<SmallInt, Pg>,
{
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        let value = *self as i16;
        <i16 as ToSql<SmallInt, Pg>>::to_sql(&value, &mut out.reborrow())
    }
}

impl FromSql<SmallInt, Pg> for PostType
where
    i16: FromSql<SmallInt, Pg>,
{
    fn from_sql(bytes: <Pg as diesel::backend::Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let database_value = i16::from_sql(bytes)?;
        PostType::from_i16(database_value).ok_or(DeserializePostTypeError.into())
    }
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, EnumIter, FromPrimitive, AsExpression, FromSqlRow, Serialize, Deserialize,
)]
#[diesel(sql_type = SmallInt)]
pub enum MimeType {
    #[serde(rename = "image/bmp")]
    BMP,
    #[serde(rename = "image/gif")]
    GIF,
    #[serde(rename = "image/jpeg")]
    JPEG,
    #[serde(rename = "image/png")]
    PNG,
    #[serde(rename = "image/webp")]
    WEBP,
    #[serde(rename = "video/mp4")]
    MP4,
    #[serde(rename = "video/mov")]
    MOV,
    #[serde(rename = "video/webm")]
    WEBM,
}

impl MimeType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BMP => "image/bmp",
            Self::GIF => "image/gif",
            Self::JPEG => "image/jpeg",
            Self::PNG => "image/png",
            Self::WEBP => "image/webp",
            Self::MP4 => "video/mp4",
            Self::MOV => "video/mov",
            Self::WEBM => "video/webm",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::BMP => "bmp",
            Self::GIF => "gif",
            Self::JPEG => "jpg",
            Self::PNG => "png",
            Self::WEBP => "webp",
            Self::MP4 => "mp4",
            Self::MOV => "mov",
            Self::WEBM => "webm",
        }
    }
}

impl std::str::FromStr for MimeType {
    type Err = ParseMimeTypeError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        MimeType::iter()
            .find(|mime_type| s == mime_type.as_str())
            .ok_or(ParseMimeTypeError)
    }
}

impl ToSql<SmallInt, Pg> for MimeType
where
    i16: ToSql<SmallInt, Pg>,
{
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        let value = *self as i16;
        <i16 as ToSql<SmallInt, Pg>>::to_sql(&value, &mut out.reborrow())
    }
}

impl FromSql<SmallInt, Pg> for MimeType
where
    i16: FromSql<SmallInt, Pg>,
{
    fn from_sql(bytes: <Pg as diesel::backend::Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let database_value = i16::from_sql(bytes)?;
        MimeType::from_i16(database_value).ok_or(DeserializeMimeTypeError.into())
    }
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, FromPrimitive, AsExpression, FromSqlRow, Serialize, Deserialize,
)]
#[diesel(sql_type = SmallInt)]
#[serde(rename_all = "lowercase")]
pub enum PostSafety {
    Safe,
    Sketchy,
    Unsafe,
}

impl ToSql<SmallInt, Pg> for PostSafety
where
    i16: ToSql<SmallInt, Pg>,
{
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        let value = *self as i16;
        <i16 as ToSql<SmallInt, Pg>>::to_sql(&value, &mut out.reborrow())
    }
}

impl FromSql<SmallInt, Pg> for PostSafety
where
    i16: FromSql<SmallInt, Pg>,
{
    fn from_sql(bytes: <Pg as diesel::backend::Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let database_value = i16::from_sql(bytes)?;
        PostSafety::from_i16(database_value).ok_or(DeserializePostSafetyError.into())
    }
}

#[derive(Debug, Error)]
#[error("Failed to mime type")]
pub struct ParseMimeTypeError;

#[derive(Debug, Error)]
#[error("Failed to parse user privilege")]
pub struct ParseUserRankError;

#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    EnumIter,
    FromPrimitive,
    AsExpression,
    FromSqlRow,
    Serialize,
    Deserialize,
)]
#[diesel(sql_type = SmallInt)]
#[serde(rename_all = "lowercase")]
pub enum UserRank {
    Anonymous,
    Restricted,
    Regular,
    Power,
    Moderator,
    Administrator,
}

impl UserRank {
    pub fn has_permission_to(self, action: &str) -> bool {
        auth::privilege_needed(action)
            .map(|required_rank| self >= required_rank)
            .unwrap_or(false)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            UserRank::Anonymous => "anonymous",
            UserRank::Restricted => "restricted",
            UserRank::Regular => "regular",
            UserRank::Power => "power",
            UserRank::Moderator => "moderator",
            UserRank::Administrator => "administrator",
        }
    }
}

impl std::str::FromStr for UserRank {
    type Err = ParseUserRankError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        UserRank::iter()
            .find(|rank| s == rank.as_str())
            .ok_or(ParseUserRankError)
    }
}

impl ToSql<SmallInt, Pg> for UserRank
where
    i16: ToSql<SmallInt, Pg>,
{
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        let value = *self as i16;
        <i16 as ToSql<SmallInt, Pg>>::to_sql(&value, &mut out.reborrow())
    }
}

impl FromSql<SmallInt, Pg> for UserRank
where
    i16: FromSql<SmallInt, Pg>,
{
    fn from_sql(bytes: <Pg as diesel::backend::Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let database_value = i16::from_sql(bytes)?;
        UserRank::from_i16(database_value).ok_or(DeserializeUserPrivilegeError.into())
    }
}

#[derive(Debug, Error)]
#[error("Failed to deserialize avatar style")]
struct DeserializeAvatarStyleError;

#[derive(Debug, Error)]
#[error("Failed to deserialize post type")]
struct DeserializePostTypeError;

#[derive(Debug, Error)]
#[error("Failed to deserialize mime type")]
struct DeserializeMimeTypeError;

#[derive(Debug, Error)]
#[error("Failed to deserialize post safety")]
struct DeserializePostSafetyError;

#[derive(Debug, Error)]
#[error("Failed to deserialize user privilege")]
struct DeserializeUserPrivilegeError;

#[cfg(test)]
mod test {
    use super::*;
    use crate::test::*;

    #[test]
    fn safety_ordering() {
        assert!(PostSafety::Safe < PostSafety::Sketchy);
        assert!(PostSafety::Sketchy < PostSafety::Unsafe);
        assert_eq!(PostSafety::Safe, PostSafety::Safe);
        assert_ne!(PostSafety::Safe, PostSafety::Unsafe);
    }

    #[test]
    fn rank_ordering() {
        assert!(UserRank::Restricted < UserRank::Regular);
        assert!(UserRank::Administrator > UserRank::Moderator);
        assert_eq!(UserRank::Regular, UserRank::Regular);
        assert_ne!(UserRank::Regular, UserRank::Moderator);
    }

    #[test]
    fn permission() {
        use_dist_config();
        test_transaction(|conn| {
            let user = create_test_user(conn, TEST_USERNAME)?;
            assert!(user.rank.has_permission_to("users:create:self"));
            assert!(user.rank.has_permission_to("posts:list"));
            assert!(!user.rank.has_permission_to("users:edit:self:rank"));
            assert!(!user.rank.has_permission_to("users:delete:any"));
            assert!(!user.rank.has_permission_to("fake:action"));
            Ok(())
        });
    }
}
