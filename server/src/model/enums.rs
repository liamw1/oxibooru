use crate::auth;
use diesel::backend::Backend;
use diesel::deserialize::FromSql;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::SmallInt;
use diesel::FromSqlRow;
use diesel::{deserialize, AsExpression};
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive, ToPrimitive};
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use thiserror::Error;

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, FromPrimitive, ToPrimitive, AsExpression, FromSqlRow, Serialize, Deserialize,
)]
#[diesel(sql_type = SmallInt)]
#[serde(rename_all = "lowercase")]
pub enum PostType {
    Image,
    Animation,
    Video,
    Flash,
    Youtube,
}

impl<DB: Backend> ToSql<SmallInt, DB> for PostType
where
    i16: ToSql<SmallInt, DB>,
{
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, DB>) -> serialize::Result {
        // I have to do this jank here to get around the fact that to_sql doesn't work when called on a temporary
        const VALUES: [i16; 5] = [0, 1, 2, 3, 4];
        VALUES[self.to_usize().unwrap()].to_sql(out)
    }
}

impl<DB: Backend> FromSql<SmallInt, DB> for PostType
where
    i16: FromSql<SmallInt, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> deserialize::Result<Self> {
        let database_value = i16::from_sql(bytes)?;
        PostType::from_i16(database_value).ok_or(DeserializePostTypeError.into())
    }
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, FromPrimitive, ToPrimitive, AsExpression, FromSqlRow, Serialize, Deserialize,
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

impl<DB: Backend> ToSql<SmallInt, DB> for MimeType
where
    i16: ToSql<SmallInt, DB>,
{
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, DB>) -> serialize::Result {
        // I have to do this jank here to get around the fact that to_sql doesn't work when called on a temporary
        const VALUES: [i16; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 8];
        VALUES[self.to_usize().unwrap()].to_sql(out)
    }
}

impl<DB: Backend> FromSql<SmallInt, DB> for MimeType
where
    i16: FromSql<SmallInt, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> deserialize::Result<Self> {
        let database_value = i16::from_sql(bytes)?;
        MimeType::from_i16(database_value).ok_or(DeserializeMimeTypeError.into())
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
    FromPrimitive,
    ToPrimitive,
    AsExpression,
    FromSqlRow,
    Serialize,
    Deserialize,
)]
#[diesel(sql_type = SmallInt)]
#[serde(rename_all = "lowercase")]
pub enum PostSafety {
    Safe,
    Sketchy,
    Unsafe,
}

impl<DB: Backend> ToSql<SmallInt, DB> for PostSafety
where
    i16: ToSql<SmallInt, DB>,
{
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, DB>) -> serialize::Result {
        // I have to do this jank here to get around the fact that to_sql doesn't work when called on a temporary
        const VALUES: [i16; 3] = [0, 1, 2];
        VALUES[self.to_usize().unwrap()].to_sql(out)
    }
}

impl<DB: Backend> FromSql<SmallInt, DB> for PostSafety
where
    i16: FromSql<SmallInt, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> deserialize::Result<Self> {
        let database_value = i16::from_sql(bytes)?;
        PostSafety::from_i16(database_value).ok_or(DeserializePostSafetyError.into())
    }
}

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
    ToPrimitive,
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

impl<DB: Backend> ToSql<SmallInt, DB> for UserRank
where
    i16: ToSql<SmallInt, DB>,
{
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, DB>) -> serialize::Result {
        // I have to do this jank here to get around the fact that to_sql doesn't work when called on a temporary
        const VALUES: [i16; 6] = [0, 1, 2, 3, 4, 5];
        VALUES[self.to_usize().unwrap()].to_sql(out)
    }
}

impl<DB: Backend> FromSql<SmallInt, DB> for UserRank
where
    i16: FromSql<SmallInt, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> deserialize::Result<Self> {
        let database_value = i16::from_sql(bytes)?;
        UserRank::from_i16(database_value).ok_or(DeserializeUserPrivilegeError.into())
    }
}

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
