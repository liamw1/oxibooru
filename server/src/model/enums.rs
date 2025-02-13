use diesel::deserialize::{self, FromSql};
use diesel::pg::Pg;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::SmallInt;
use diesel::{AsExpression, FromSqlRow};
use image::ImageFormat;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::path::Path;
use strum::{Display, EnumCount, EnumString, FromRepr, IntoStaticStr};
use thiserror::Error;

/// In general, the order of these enums should not be changed.
/// They are encoded in the database as an integer, so changing
/// the underlying representation of an enum changes its meaning.
///
/// New enum variants should therefore always be appended at the end.

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

impl Default for AvatarStyle {
    fn default() -> Self {
        Self::Gravatar
    }
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
}

impl From<MimeType> for PostType {
    fn from(value: MimeType) -> Self {
        match value {
            MimeType::Bmp => Self::Image,
            MimeType::Gif => Self::Animation,
            MimeType::Jpeg => Self::Image,
            MimeType::Png => Self::Image,
            MimeType::Webp => Self::Image,
            MimeType::Mp4 => Self::Video,
            MimeType::Mov => Self::Video,
            MimeType::Webm => Self::Video,
            MimeType::Swf => Self::Flash,
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
    Debug,
    Display,
    Copy,
    Clone,
    PartialEq,
    Eq,
    EnumString,
    FromRepr,
    IntoStaticStr,
    AsExpression,
    FromSqlRow,
    Serialize,
    Deserialize,
)]
#[diesel(sql_type = SmallInt)]
#[repr(i16)]
pub enum MimeType {
    #[serde(rename = "image/bmp")]
    #[strum(serialize = "image/bmp")]
    Bmp,
    #[serde(rename = "image/gif")]
    #[strum(serialize = "image/gif")]
    Gif,
    #[serde(rename = "image/jpeg")]
    #[strum(serialize = "image/jpeg")]
    Jpeg,
    #[serde(rename = "image/png")]
    #[strum(serialize = "image/png")]
    Png,
    #[serde(rename = "image/webp")]
    #[strum(serialize = "image/webp")]
    Webp,
    #[serde(rename = "video/mp4")]
    #[strum(serialize = "video/mp4")]
    Mp4,
    #[serde(rename = "video/quicktime")]
    #[strum(serialize = "video/quicktime")]
    Mov,
    #[serde(rename = "video/webm")]
    #[strum(serialize = "video/webm")]
    Webm,
    #[serde(rename = "application/x-shockwave-flash")]
    #[strum(serialize = "application/x-shockwave-flash")]
    Swf,
}

impl MimeType {
    pub fn from_extension(extension: &str) -> Result<Self, ParseExtensionError> {
        match extension {
            "bmp" | "BMP" => Ok(Self::Bmp),
            "gif" | "GIF" => Ok(Self::Gif),
            "jpg" | "jpeg" | "JPG" | "JPEG" => Ok(Self::Jpeg),
            "png" | "PNG" => Ok(Self::Png),
            "webp" | "WEBP" => Ok(Self::Webp),
            "mp4" | "MP4" => Ok(Self::Mp4),
            "mov" | "MOV" => Ok(Self::Mov),
            "webm" | "WEBM" => Ok(Self::Webm),
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
            Self::Bmp => "bmp",
            Self::Gif => "gif",
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
            Self::Mp4 => "mp4",
            Self::Mov => "mov",
            Self::Webm => "webm",
            Self::Swf => "swf",
        }
    }

    pub fn to_image_format(self) -> Option<ImageFormat> {
        match self {
            MimeType::Bmp => Some(ImageFormat::Bmp),
            MimeType::Gif => Some(ImageFormat::Gif),
            MimeType::Jpeg => Some(ImageFormat::Jpeg),
            MimeType::Png => Some(ImageFormat::Png),
            MimeType::Webp => Some(ImageFormat::WebP),
            MimeType::Mov | MimeType::Mp4 | MimeType::Webm | MimeType::Swf => None,
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

#[derive(Copy, Clone, EnumCount, FromRepr, IntoStaticStr, Deserialize)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum PostFlag {
    Loop,
    Sound,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default, AsExpression, FromSqlRow)]
#[diesel(sql_type = SmallInt)]
pub struct PostFlags {
    flags: u16, // Bit mask of possible flags
}

impl PostFlags {
    pub const fn new() -> Self {
        Self { flags: 0 }
    }

    pub const fn new_with(flag: PostFlag) -> Self {
        Self {
            flags: 1 << flag as u16,
        }
    }

    pub fn from_slice(flags: &[PostFlag]) -> Self {
        Self {
            flags: flags.iter().fold(0, |flags, &flag| flags | 1 << flag as u16),
        }
    }

    pub fn add(&mut self, flag: PostFlag) {
        self.flags |= 1 << flag as u16;
    }
}

impl std::ops::BitOrAssign for PostFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.flags |= rhs.flags;
    }
}

impl ToSql<SmallInt, Pg> for PostFlags
where
    i16: ToSql<SmallInt, Pg>,
{
    fn to_sql(&self, out: &mut Output<Pg>) -> serialize::Result {
        let value = self.flags as i16;
        <i16 as ToSql<SmallInt, Pg>>::to_sql(&value, &mut out.reborrow())
    }
}

impl FromSql<SmallInt, Pg> for PostFlags
where
    i16: FromSql<SmallInt, Pg>,
{
    fn from_sql(bytes: <Pg as diesel::backend::Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        i16::from_sql(bytes).map(|database_value| Self {
            flags: database_value as u16,
        })
    }
}

impl Serialize for PostFlags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        const _: () = assert!(PostFlag::COUNT <= 16);

        let flags: Vec<&'static str> = (0..PostFlag::COUNT)
            .filter(|f| self.flags & (1 << f) != 0) // Check if flag is set
            .map(|f| PostFlag::from_repr(f).unwrap().into())
            .collect();
        flags.serialize(serializer)
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
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

#[derive(Debug, Display)]
#[strum(serialize_all = "lowercase")]
pub enum ResourceType {
    Comment,
    Pool,
    #[strum(serialize = "pool category")]
    PoolCategory,
    Post,
    Tag,
    #[strum(serialize = "tag category")]
    TagCategory,
    #[strum(serialize = "tag implication")]
    TagImplication,
    #[strum(serialize = "tag suggestion")]
    TagSuggestion,
    User,
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
}
