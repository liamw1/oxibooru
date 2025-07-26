use crate::api::ApiResult;
use crate::auth::Client;
use crate::model::enums::{ResourceOperation, ResourceType};
use crate::model::snapshot::NewSnapshot;
use crate::schema::{pool_category, pool_name, pool_post};
use crate::string::SmallString;
use crate::{api, snapshot};
use diesel::prelude::*;
use serde::Serialize;
use serde_json::json;

#[derive(Clone, Serialize)]
pub struct SnapshotData {
    pub category: SmallString,
    pub names: Vec<SmallString>,
    pub posts: Vec<i64>,
}

impl SnapshotData {
    pub fn retrieve(conn: &mut PgConnection, pool_id: i64, category_id: i64) -> QueryResult<Self> {
        let category = pool_category::table
            .find(category_id)
            .select(pool_category::name)
            .first(conn)?;
        let names = pool_name::table
            .select(pool_name::name)
            .filter(pool_name::pool_id.eq(pool_id))
            .load(conn)?;
        let posts = pool_post::table
            .select(pool_post::post_id)
            .filter(pool_post::pool_id.eq(pool_id))
            .load(conn)?;
        Ok(Self { category, names, posts })
    }

    fn sort_fields(&mut self) {
        self.names.sort_unstable();
        self.posts.sort_unstable();
    }
}

pub fn creation_snapshot(
    conn: &mut PgConnection,
    client: Client,
    pool_id: i64,
    pool_data: SnapshotData,
) -> ApiResult<()> {
    unary_snapshot(conn, client, pool_id, pool_data, ResourceOperation::Created)
}

pub fn merge_snapshot(
    conn: &mut PgConnection,
    client: Client,
    absorbed_pool_id: i64,
    merge_to_pool_id: i64,
) -> QueryResult<()> {
    let data = json!([ResourceType::Pool, merge_to_pool_id]);
    NewSnapshot {
        user_id: client.id,
        operation: ResourceOperation::Merged,
        resource_type: ResourceType::Pool,
        resource_id: absorbed_pool_id.into(),
        data,
    }
    .insert(conn)
}

pub fn modification_snapshot(
    conn: &mut PgConnection,
    client: Client,
    pool_id: i64,
    mut old: SnapshotData,
    mut new: SnapshotData,
) -> ApiResult<()> {
    old.sort_fields();
    new.sort_fields();
    let old_data = serde_json::to_value(old)?;
    let new_data = serde_json::to_value(new)?;
    if let Some(data) = snapshot::value_diff(old_data, new_data) {
        NewSnapshot {
            user_id: client.id,
            operation: ResourceOperation::Modified,
            resource_type: ResourceType::Pool,
            resource_id: pool_id.into(),
            data,
        }
        .insert(conn)?;
    }
    Ok(())
}

pub fn deletion_snapshot(
    conn: &mut PgConnection,
    client: Client,
    pool_id: i64,
    pool_data: SnapshotData,
) -> ApiResult<()> {
    unary_snapshot(conn, client, pool_id, pool_data, ResourceOperation::Deleted)
}

fn unary_snapshot(
    conn: &mut PgConnection,
    client: Client,
    pool_id: i64,
    mut pool_data: SnapshotData,
    operation: ResourceOperation,
) -> ApiResult<()> {
    pool_data.sort_fields();
    serde_json::to_value(pool_data)
        .map_err(api::Error::from)
        .and_then(|data| {
            NewSnapshot {
                user_id: client.id,
                operation,
                resource_type: ResourceType::Pool,
                resource_id: pool_id.into(),
                data,
            }
            .insert(conn)
            .map_err(api::Error::from)
        })
}
