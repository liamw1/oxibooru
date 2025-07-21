use crate::model::enums::{ResourceType, ResourceOperation};
use crate::model::snapshot::Snapshot;
use crate::resource;
use crate::resource::BoolFill;
use crate::schema::{snapshot, user};
use crate::time::DateTime;
use diesel::prelude::*;
use serde::Serialize;
use serde_json::Value;
use serde_with::skip_serializing_none;
use strum::{EnumString, EnumTable};

#[derive(Clone, Copy, EnumString, EnumTable)]
#[strum(serialize_all = "camelCase")]
pub enum Field {
    User,
    Operation,
    Type,
    Id,
    Data,
    Time,
}

impl BoolFill for FieldTable<bool> {
    fn filled(val: bool) -> Self {
        Self::filled(val)
    }
}

#[skip_serializing_none]
#[derive(Serialize)]
pub struct SnapshotInfo {
    user: Option<Option<String>>,
    operation: Option<ResourceOperation>,
    #[serde(rename(serialize = "type"))]
    resource_type: Option<ResourceType>,
    #[serde(rename(serialize = "id"))]
    resource_id: Option<i64>,
    data: Option<Value>,
    time: Option<DateTime>,
}

impl SnapshotInfo {
    pub fn new_batch(
        conn: &mut PgConnection,
        snapshots: Vec<Snapshot>,
        fields: &FieldTable<bool>,
    ) -> QueryResult<Vec<Self>> {
        let batch_size = snapshots.len();
        let mut users = resource::retrieve(fields[Field::User], || get_users(conn, &snapshots))?;
        resource::check_batch_results(batch_size, users.len());

        let results = snapshots
            .into_iter()
            .rev()
            .map(|snapshot| Self {
                user: users.pop(),
                operation: fields[Field::Operation].then_some(snapshot.operation),
                resource_type: fields[Field::Type].then_some(snapshot.resource_type),
                resource_id: fields[Field::Id].then_some(snapshot.resource_id),
                data: fields[Field::Data].then_some(snapshot.data),
                time: fields[Field::Time].then_some(snapshot.creation_time),
            })
            .collect::<Vec<_>>();
        Ok(results.into_iter().rev().collect())
    }

    pub fn new_batch_from_ids(
        conn: &mut PgConnection,
        snapshot_ids: Vec<i64>,
        fields: &FieldTable<bool>,
    ) -> QueryResult<Vec<Self>> {
        let unordered_snapshots = snapshot::table.filter(snapshot::id.eq_any(&snapshot_ids)).load(conn)?;
        let snapshots = resource::order_as(unordered_snapshots, &snapshot_ids);
        Self::new_batch(conn, snapshots, fields)
    }
}

fn get_users(conn: &mut PgConnection, snapshots: &[Snapshot]) -> QueryResult<Vec<Option<String>>> {
    let snapshot_ids: Vec<_> = snapshots.iter().filter_map(|snapshot| snapshot.user_id).collect();
    user::table
        .select((user::id, user::name))
        .filter(user::id.eq_any(snapshot_ids))
        .load::<(i64, String)>(conn)
        .map(|user_info| {
            resource::order_like(user_info, snapshots, |&(id, _)| id)
                .into_iter()
                .map(|user_info| user_info.map(|(_, username)| username))
                .collect()
        })
}
