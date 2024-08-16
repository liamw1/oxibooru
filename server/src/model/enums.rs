use diesel::deserialize::{self, FromSql};
use diesel::pg::Pg;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::SmallInt;
use diesel::{AsExpression, FromSqlRow};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::path::Path;
use strum::{EnumIter, EnumString, FromRepr};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
#[error("{extenstion} is not a supported file extension")]
pub struct ParseExtensionError {
    extenstion: String,
}

#[derive(Debug, Error)]
#[error("Cannot convert None to Score")]
pub struct FromRatingError;

#[derive(Debug, Copy, Clone, PartialEq, Eq, FromRepr, AsExpression, FromSqlRow, Serialize, Deserialize)]
#[diesel(sql_type = SmallInt)]
#[serde(rename_all = "lowercase")]
#[repr(i16)]
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
        AvatarStyle::from_repr(database_value).ok_or(DeserializeAvatarStyleError.into())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, FromRepr, EnumString, AsExpression, FromSqlRow, Serialize, Deserialize)]
#[diesel(sql_type = SmallInt)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[repr(i16)]
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
        PostType::from_repr(database_value).ok_or(DeserializePostTypeError.into())
    }
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, EnumIter, EnumString, FromRepr, AsExpression, FromSqlRow, Serialize, Deserialize,
)]
#[diesel(sql_type = SmallInt)]
#[repr(i16)]
pub enum MimeType {
    #[serde(rename = "image/bmp")]
    #[strum(serialize = "image/bmp")]
    BMP,
    #[serde(rename = "image/gif")]
    #[strum(serialize = "image/gif")]
    GIF,
    #[serde(rename = "image/jpeg")]
    #[strum(serialize = "image/jpeg")]
    JPEG,
    #[serde(rename = "image/png")]
    #[strum(serialize = "image/png")]
    PNG,
    #[serde(rename = "image/webp")]
    #[strum(serialize = "image/webp")]
    WEBP,
    #[serde(rename = "video/mp4")]
    #[strum(serialize = "video/mp4")]
    MP4,
    #[serde(rename = "video/mov")]
    #[strum(serialize = "video/mov")]
    MOV,
    #[serde(rename = "video/webm")]
    #[strum(serialize = "video/webm")]
    WEBM,
}

impl MimeType {
    pub fn from_extension(extension: &str) -> Result<Self, ParseExtensionError> {
        match extension {
            "bmp" | "BMP" => Ok(Self::BMP),
            "gif" | "GIF" => Ok(Self::GIF),
            "jpg" | "jpeg" | "JPG" | "JPEG" => Ok(Self::JPEG),
            "png" | "PNG" => Ok(Self::PNG),
            "webp" | "WEBP" => Ok(Self::WEBP),
            "mp4" | "MP4" => Ok(Self::MP4),
            "mov" | "MOV" => Ok(Self::MOV),
            "webm" | "WEBM" => Ok(Self::WEBM),
            _ => Err(ParseExtensionError {
                extenstion: String::from(extension),
            }),
        }
    }

    pub fn from_path(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_string_lossy();
        Self::from_extension(&extension).ok()
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
        MimeType::from_repr(database_value).ok_or(DeserializeMimeTypeError.into())
    }
}

#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    EnumString,
    FromRepr,
    AsExpression,
    FromSqlRow,
    Serialize,
    Deserialize,
)]
#[diesel(sql_type = SmallInt)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[repr(i16)]
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
        PostSafety::from_repr(database_value).ok_or(DeserializePostSafetyError.into())
    }
}

#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    EnumString,
    FromRepr,
    AsExpression,
    FromSqlRow,
    Serialize,
    Deserialize,
)]
#[diesel(sql_type = SmallInt)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[repr(i16)]
pub enum UserRank {
    Anonymous,
    Restricted,
    Regular,
    Power,
    Moderator,
    Administrator,
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
        UserRank::from_repr(database_value).ok_or(DeserializeUserPrivilegeError.into())
    }
}

#[derive(Debug, Copy, Clone, Serialize_repr, Deserialize_repr)]
#[repr(i16)]
pub enum Rating {
    Dislike = -1,
    None = 0,
    Like = 1,
}

impl Default for Rating {
    fn default() -> Self {
        Self::None
    }
}

impl From<Score> for Rating {
    fn from(value: Score) -> Self {
        match value {
            Score::Dislike => Self::Dislike,
            Score::Like => Self::Like,
        }
    }
}

#[derive(Debug, Copy, Clone, FromRepr, AsExpression, FromSqlRow)]
#[diesel(sql_type = SmallInt)]
#[repr(i16)]
pub enum Score {
    Dislike = -1,
    Like = 1,
}

impl TryFrom<Rating> for Score {
    type Error = FromRatingError;
    fn try_from(value: Rating) -> Result<Self, Self::Error> {
        match value {
            Rating::None => Err(FromRatingError),
            Rating::Dislike => Ok(Self::Dislike),
            Rating::Like => Ok(Self::Like),
        }
    }
}

impl ToSql<SmallInt, Pg> for Score
where
    i16: ToSql<SmallInt, Pg>,
{
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        let value = *self as i16;
        <i16 as ToSql<SmallInt, Pg>>::to_sql(&value, &mut out.reborrow())
    }
}

impl FromSql<SmallInt, Pg> for Score
where
    i16: FromSql<SmallInt, Pg>,
{
    fn from_sql(bytes: <Pg as diesel::backend::Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let database_value = i16::from_sql(bytes)?;
        Score::from_repr(database_value).ok_or(DeserializeScoreError.into())
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

#[derive(Debug, Error)]
#[error("Failed to deserialize score")]
struct DeserializeScoreError;

#[cfg(test)]
mod test {
    use super::*;
    use crate::{config, test::*};

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
            assert!(user.rank >= config::privileges().user_create_self);
            assert!(user.rank >= config::privileges().post_list);
            assert!(user.rank < config::privileges().user_edit_self_rank);
            assert!(user.rank < config::privileges().user_delete_any);
            Ok(())
        });
    }
}
