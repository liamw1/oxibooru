use crate::auth::Client;
use crate::model::enums::{ResourceOperation, ResourceType};
use crate::model::pool_category::PoolCategory;
use crate::model::snapshot::NewSnapshot;
use crate::snapshot;
use diesel::{PgConnection, QueryResult};
use serde_json::{Value, json};

pub fn creation_snapshot(conn: &mut PgConnection, client: Client, pool_category: &PoolCategory) -> QueryResult<()> {
    unary_snapshot(conn, client, pool_category, ResourceOperation::Created)
}

pub fn set_default_snapshot(
    conn: &mut PgConnection,
    client: Client,
    old_default: &PoolCategory,
    new_default: &PoolCategory,
) -> QueryResult<()> {
    if old_default.id == new_default.id {
        return Ok(());
    }

    let defaulted_data = json!({"default": true});
    let non_defaulted_data = json!({"default": false});

    let old_default_diff = snapshot::value_diff(defaulted_data.clone(), non_defaulted_data.clone()).unwrap();
    NewSnapshot {
        user_id: client.id,
        operation: ResourceOperation::Modified,
        resource_type: ResourceType::PoolCategory,
        resource_id: old_default.name.clone(),
        data: old_default_diff,
    }
    .insert(conn)?;

    let new_default_diff = snapshot::value_diff(non_defaulted_data, defaulted_data).unwrap();
    NewSnapshot {
        user_id: client.id,
        operation: ResourceOperation::Modified,
        resource_type: ResourceType::PoolCategory,
        resource_id: new_default.name.clone(),
        data: new_default_diff,
    }
    .insert(conn)?;
    Ok(())
}

pub fn modification_snapshot(
    conn: &mut PgConnection,
    client: Client,
    old: &PoolCategory,
    new: &PoolCategory,
) -> QueryResult<()> {
    assert_eq!(old.id, new.id);

    let old_data = snapshot_data(old);
    let new_data = snapshot_data(new);
    if let Some(data) = snapshot::value_diff(old_data, new_data) {
        NewSnapshot {
            user_id: client.id,
            operation: ResourceOperation::Modified,
            resource_type: ResourceType::PoolCategory,
            resource_id: old.name.clone(),
            data,
        }
        .insert(conn)?;
    }
    Ok(())
}

pub fn deletion_snapshot(conn: &mut PgConnection, client: Client, pool_category: &PoolCategory) -> QueryResult<()> {
    unary_snapshot(conn, client, pool_category, ResourceOperation::Deleted)
}

fn snapshot_data(pool_category: &PoolCategory) -> Value {
    json!({
        "name": pool_category.name,
        "color": pool_category.color,
        "default": pool_category.id == 0,
    })
}

fn unary_snapshot(
    conn: &mut PgConnection,
    client: Client,
    pool_category: &PoolCategory,
    operation: ResourceOperation,
) -> QueryResult<()> {
    NewSnapshot {
        user_id: client.id,
        operation,
        resource_type: ResourceType::PoolCategory,
        resource_id: pool_category.name.clone(),
        data: snapshot_data(pool_category),
    }
    .insert(conn)
}
