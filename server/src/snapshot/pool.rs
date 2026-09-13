use crate::api::error::{ApiError, ApiResult};
use crate::auth::Client;
use crate::model::enums::{ResourceOperation, ResourceType};
use crate::model::pool::Pool;
use crate::model::pool_category::PoolCategory;
use crate::model::snapshot::NewSnapshot;
use crate::schema::{pool_category, pool_name, pool_post};
use crate::snapshot;
use crate::string::{LargeString, SmallString};
use diesel::{ExpressionMethods, Insertable, PgConnection, QueryDsl, QueryResult, RunQueryDsl};
use serde::Serialize;
use serde_json::json;

#[derive(Clone, Serialize)]
pub struct SnapshotData {
    pub description: LargeString,
    pub category: SmallString,
    pub names: Vec<SmallString>,
    pub posts: Vec<i64>,
}

impl SnapshotData {
    pub fn retrieve(conn: &mut PgConnection, pool: Pool) -> QueryResult<Self> {
        let category = pool_category::table
            .find(pool.category_id)
            .select(pool_category::name)
            .first(conn)?;
        let names = pool_name::table
            .select(pool_name::name)
            .filter(pool_name::pool_id.eq(pool.id))
            .load(conn)?;
        let posts = pool_post::table
            .select(pool_post::post_id)
            .filter(pool_post::pool_id.eq(pool.id))
            .load(conn)?;

        Ok(Self {
            description: pool.description,
            category,
            names,
            posts,
        })
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

pub fn new_name_snapshots(conn: &mut PgConnection, client: Client, new_names: Vec<SmallString>) -> ApiResult<usize> {
    let default_category_name: SmallString = pool_category::table
        .select(pool_category::name)
        .filter(PoolCategory::is_default())
        .first(conn)?;
    let new_snapshots: Vec<NewSnapshot> = new_names
        .into_iter()
        .map(|name| SnapshotData {
            description: LargeString::default(),
            category: default_category_name.clone(),
            names: vec![name],
            posts: Vec::new(),
        })
        .map(|pool_data| {
            let resource_id = pool_data
                .names
                .first()
                .expect("A pool must have at least one name")
                .clone();
            serde_json::to_value(pool_data).map(|data| NewSnapshot {
                user_id: client.id,
                operation: ResourceOperation::Created,
                resource_type: ResourceType::Pool,
                resource_id,
                data,
            })
        })
        .collect::<Result<_, _>>()?;
    new_snapshots
        .insert_into(crate::schema::snapshot::table)
        .execute(conn)
        .map_err(ApiError::from)
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
        .map_err(ApiError::from)
        .and_then(|data| {
            NewSnapshot {
                user_id: client.id,
                operation,
                resource_type: ResourceType::Pool,
                resource_id: pool_id.into(),
                data,
            }
            .insert(conn)
            .map_err(ApiError::from)
        })
}
