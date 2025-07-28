use diesel::deserialize::{self, FromSql};
use diesel::pg::{Pg, PgValue};
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::SmallInt;
use diesel::{AsExpression, FromSqlRow};
use image::ImageFormat;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::ops::{BitOr, BitOrAssign};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRepr, AsExpression, FromSqlRow, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[diesel(sql_type = SmallInt)]
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

impl ToSql<SmallInt, Pg> for AvatarStyle {
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Pg>) -> serialize::Result {
        // SAFETY: AvatarStyle is repr(i16) so a valid AvatarStyle is a valid i16
        let value: &'a i16 = unsafe { &*std::ptr::from_ref(self).cast() };
        <i16 as ToSql<SmallInt, Pg>>::to_sql(value, out)
    }
}

impl FromSql<SmallInt, Pg> for AvatarStyle {
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        let database_value = i16::from_sql(value)?;
        AvatarStyle::from_repr(database_value).ok_or("Failed to deserialize avatar style".into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString, FromRepr, AsExpression, FromSqlRow, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[diesel(sql_type = SmallInt)]
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
            MimeType::Bmp | MimeType::Jpeg | MimeType::Png | MimeType::Webp => Self::Image,
            MimeType::Gif => Self::Animation,
            MimeType::Mp4 | MimeType::Mov | MimeType::Webm => Self::Video,
            MimeType::Swf => Self::Flash,
        }
    }
}

impl ToSql<SmallInt, Pg> for PostType {
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Pg>) -> serialize::Result {
        // SAFETY: PostType is repr(i16) so a valid PostType is a valid i16
        let value: &'a i16 = unsafe { &*std::ptr::from_ref(self).cast() };
        <i16 as ToSql<SmallInt, Pg>>::to_sql(value, out)
    }
}

impl FromSql<SmallInt, Pg> for PostType {
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        let database_value = i16::from_sql(value)?;
        PostType::from_repr(database_value).ok_or("Failed to deserialize post type".into())
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
            "swf" | "SWF" => Ok(Self::Swf),
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

impl ToSql<SmallInt, Pg> for MimeType {
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Pg>) -> serialize::Result {
        // SAFETY: MimeType is repr(i16) so a valid MimeType is a valid i16
        let value: &'a i16 = unsafe { &*std::ptr::from_ref(self).cast() };
        <i16 as ToSql<SmallInt, Pg>>::to_sql(value, out)
    }
}

impl FromSql<SmallInt, Pg> for MimeType {
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        let database_value = i16::from_sql(value)?;
        MimeType::from_repr(database_value).ok_or("Failed to deserialize mime type".into())
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
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[diesel(sql_type = SmallInt)]
#[repr(i16)]
pub enum PostSafety {
    Safe,
    Sketchy,
    Unsafe,
}

impl ToSql<SmallInt, Pg> for PostSafety {
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Pg>) -> serialize::Result {
        // SAFETY: PostSafety is repr(i16) so a valid PostSafety is a valid i16
        let value: &'a i16 = unsafe { &*std::ptr::from_ref(self).cast() };
        <i16 as ToSql<SmallInt, Pg>>::to_sql(value, out)
    }
}

impl FromSql<SmallInt, Pg> for PostSafety {
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        let database_value = i16::from_sql(value)?;
        PostSafety::from_repr(database_value).ok_or("Failed to deserialize post safety".into())
    }
}

#[derive(Clone, Copy, EnumCount, EnumString, FromRepr, IntoStaticStr, Deserialize)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum PostFlag {
    Loop,
    Sound,
}

impl From<PostFlag> for u16 {
    fn from(value: PostFlag) -> Self {
        1 << value as u16
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, AsExpression, FromSqlRow)]
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
        flags.iter().fold(Self::new(), |flags, &flag| flags | flag)
    }
}

impl From<PostFlags> for u16 {
    fn from(value: PostFlags) -> Self {
        value.flags
    }
}

impl<T: Into<u16>> BitOr<T> for PostFlags {
    type Output = Self;
    fn bitor(self, rhs: T) -> Self::Output {
        Self {
            flags: self.flags | rhs.into(),
        }
    }
}

impl<T: Into<u16>> BitOrAssign<T> for PostFlags {
    fn bitor_assign(&mut self, rhs: T) {
        self.flags |= rhs.into();
    }
}

impl ToSql<SmallInt, Pg> for PostFlags {
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Pg>) -> serialize::Result {
        // SAFETY: A u16 bitpattern is always a valid i16 bitpattern
        let value: &'a i16 = unsafe { &*std::ptr::from_ref(&self.flags).cast() };
        <i16 as ToSql<SmallInt, Pg>>::to_sql(value, out)
    }
}

impl FromSql<SmallInt, Pg> for PostFlags {
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        i16::from_sql(value).map(|database_value| Self {
            flags: database_value.cast_unsigned(),
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
    Default,
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
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[diesel(sql_type = SmallInt)]
#[repr(i16)]
pub enum UserRank {
    Anonymous,
    Restricted,
    #[default]
    Regular,
    Power,
    Moderator,
    Administrator,
}

impl ToSql<SmallInt, Pg> for UserRank {
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Pg>) -> serialize::Result {
        // SAFETY: UserRank is repr(i16) so a valid UserRank is a valid i16
        let value: &'a i16 = unsafe { &*std::ptr::from_ref(self).cast() };
        <i16 as ToSql<SmallInt, Pg>>::to_sql(value, out)
    }
}

impl FromSql<SmallInt, Pg> for UserRank {
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        let database_value = i16::from_sql(value)?;
        UserRank::from_repr(database_value).ok_or("Failed to deserialize user privilege".into())
    }
}

#[derive(Debug, Clone, Copy, Serialize_repr, Deserialize_repr)]
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

#[derive(Debug, Clone, Copy, FromRepr, AsExpression, FromSqlRow)]
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

impl ToSql<SmallInt, Pg> for Score {
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Pg>) -> serialize::Result {
        // SAFETY: Score is repr(i16) so a valid Score is a valid i16
        let value: &'a i16 = unsafe { &*std::ptr::from_ref(self).cast() };
        <i16 as ToSql<SmallInt, Pg>>::to_sql(value, out)
    }
}

impl FromSql<SmallInt, Pg> for Score {
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        let database_value = i16::from_sql(value)?;
        Score::from_repr(database_value).ok_or("Failed to deserialize score".into())
    }
}

#[derive(Debug, Clone, Copy, EnumString, FromRepr, AsExpression, FromSqlRow, Serialize)]
#[serde(rename_all = "snake_case")]
#[diesel(sql_type = SmallInt)]
#[repr(i16)]
pub enum ResourceOperation {
    Created,
    Modified,
    Merged,
    Deleted,
}

impl ToSql<SmallInt, Pg> for ResourceOperation {
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Pg>) -> serialize::Result {
        // SAFETY: Score is repr(i16) so a valid ResourceOperation is a valid i16
        let value: &'a i16 = unsafe { &*std::ptr::from_ref(self).cast() };
        <i16 as ToSql<SmallInt, Pg>>::to_sql(value, out)
    }
}

impl FromSql<SmallInt, Pg> for ResourceOperation {
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        let database_value = i16::from_sql(value)?;
        ResourceOperation::from_repr(database_value).ok_or("Failed to deserialize resource operation".into())
    }
}

#[derive(Debug, Display, Clone, Copy, EnumString, FromRepr, AsExpression, FromSqlRow, Serialize)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[diesel(sql_type = SmallInt)]
#[repr(i16)]
pub enum ResourceType {
    Comment,
    Pool,
    PoolCategory,
    Post,
    Tag,
    TagCategory,
    TagImplication,
    TagSuggestion,
    User,
}

impl ToSql<SmallInt, Pg> for ResourceType {
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Pg>) -> serialize::Result {
        // SAFETY: Score is repr(i16) so a valid ResourceType is a valid i16
        let value: &'a i16 = unsafe { &*std::ptr::from_ref(self).cast() };
        <i16 as ToSql<SmallInt, Pg>>::to_sql(value, out)
    }
}

impl FromSql<SmallInt, Pg> for ResourceType {
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        let database_value = i16::from_sql(value)?;
        ResourceType::from_repr(database_value).ok_or("Failed to deserialize resource type".into())
    }
}

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
