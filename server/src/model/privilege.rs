use diesel::backend::Backend;
use diesel::deserialize::FromSql;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::SmallInt;
use diesel::FromSqlRow;
use diesel::{deserialize, AsExpression};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, AsExpression, FromSqlRow, FromPrimitive)]
#[diesel(sql_type = SmallInt)]
pub enum UserPrivilege {
    Anonymous,
    Restricted,
    Regular,
    Power,
    Moderator,
    Administrator,
}

impl<DB: Backend> ToSql<SmallInt, DB> for UserPrivilege
where
    i16: ToSql<SmallInt, DB>,
{
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, DB>) -> serialize::Result {
        // I have to do this jank here to get around the fact that to_sql doesn't work when called on a temporary
        const VALUES: [i16; 6] = [0, 1, 2, 3, 4, 5];
        VALUES[*self as usize].to_sql(out)
    }
}

impl<DB: Backend> FromSql<SmallInt, DB> for UserPrivilege
where
    i16: FromSql<SmallInt, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> deserialize::Result<Self> {
        let database_value = i16::from_sql(bytes)?;
        UserPrivilege::from_i16(database_value).ok_or("Invalid privilege level".into())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn privilege_ordering() {
        assert!(UserPrivilege::Restricted < UserPrivilege::Regular);
        assert!(UserPrivilege::Administrator > UserPrivilege::Moderator);
        assert_eq!(UserPrivilege::Regular, UserPrivilege::Regular);
        assert_ne!(UserPrivilege::Regular, UserPrivilege::Moderator);
    }
}
