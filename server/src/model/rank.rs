use crate::auth;
use diesel::backend::Backend;
use diesel::deserialize::FromSql;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::SmallInt;
use diesel::FromSqlRow;
use diesel::{deserialize, AsExpression};
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive, ToPrimitive};
use thiserror::Error;

#[derive(Debug, Error)]
#[error("Failed to parse user privilege")]
pub struct ParseUserPrivilegeError;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, AsExpression, FromSqlRow, FromPrimitive, ToPrimitive)]
#[diesel(sql_type = SmallInt)]
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
}

impl std::string::ToString for UserRank {
    fn to_string(&self) -> String {
        match self {
            UserRank::Anonymous => "anonymous",
            UserRank::Restricted => "restricted",
            UserRank::Regular => "regular",
            UserRank::Power => "power",
            UserRank::Moderator => "moderator",
            UserRank::Administrator => "administrator",
        }
        .to_string()
    }
}

impl std::str::FromStr for UserRank {
    type Err = ParseUserPrivilegeError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "anonymous" => Ok(UserRank::Anonymous),
            "restricted" => Ok(UserRank::Restricted),
            "regular" => Ok(UserRank::Regular),
            "power" => Ok(UserRank::Power),
            "moderator" => Ok(UserRank::Moderator),
            "administrator" => Ok(UserRank::Administrator),
            _ => Err(ParseUserPrivilegeError),
        }
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
#[error("Failed to deserialize user privilege")]
struct DeserializeUserPrivilegeError;

#[cfg(test)]
mod test {
    use super::*;
    use crate::test::*;

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
