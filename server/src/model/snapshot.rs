use crate::model::enums::{ResourceOperation, ResourceType};
use crate::model::user::User;
use crate::schema::snapshot;
use crate::string::SmallString;
use crate::time::DateTime;
use diesel::pg::Pg;
use diesel::prelude::*;
use serde_json::Value;

#[derive(Insertable)]
#[diesel(table_name = snapshot)]
#[diesel(check_for_backend(Pg))]
pub struct NewSnapshot {
    pub user_id: Option<i64>,
    pub operation: ResourceOperation,
    pub resource_type: ResourceType,
    pub resource_id: SmallString,
    pub data: Value,
}

impl NewSnapshot {
    /// Inserts `self` into `snapshot` table
    pub fn insert(self, conn: &mut PgConnection) -> QueryResult<()> {
        self.insert_into(snapshot::table).execute(conn).map(|_| ())
    }
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(User))]
#[diesel(table_name = snapshot)]
#[diesel(check_for_backend(Pg))]
pub struct Snapshot {
    pub id: i64,
    pub user_id: Option<i64>,
    pub operation: ResourceOperation,
    pub resource_type: ResourceType,
    pub resource_id: SmallString,
    pub data: Value,
    pub creation_time: DateTime,
}
