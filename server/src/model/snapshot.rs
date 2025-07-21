use crate::model::enums::{ResourceType, SnapshotOperation};
use crate::model::user::User;
use crate::schema::snapshot;
use crate::time::DateTime;
use diesel::pg::Pg;
use diesel::prelude::*;
use serde_json::Value;

#[derive(Insertable)]
#[diesel(table_name = snapshot)]
#[diesel(check_for_backend(Pg))]
pub struct NewSnapshot {
    pub user_id: Option<i64>,
    pub operation: SnapshotOperation,
    pub resource_type: ResourceType,
    pub resource_id: i64,
    pub data: Value,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(User))]
#[diesel(table_name = snapshot)]
#[diesel(check_for_backend(Pg))]
pub struct Snapshot {
    pub id: i64,
    pub user_id: Option<i64>,
    pub operation: SnapshotOperation,
    pub resource_type: ResourceType,
    pub resource_id: i64,
    pub data: Value,
    pub creation_time: DateTime,
}
