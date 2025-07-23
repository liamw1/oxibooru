use crate::auth::Client;
use crate::model::enums::{ResourceOperation, ResourceType};
use crate::model::snapshot::NewSnapshot;
use crate::model::tag_category::TagCategory;
use crate::snapshot;
use diesel::{PgConnection, QueryResult};
use serde_json::{Value, json};

pub fn creation_snapshot(conn: &mut PgConnection, client: Client, tag_category: &TagCategory) -> QueryResult<()> {
    unary_snapshot(conn, client, tag_category, ResourceOperation::Created)
}

pub fn set_default_snapshot(
    conn: &mut PgConnection,
    client: Client,
    old_default: &TagCategory,
    new_default: &TagCategory,
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
        resource_type: ResourceType::TagCategory,
        resource_id: old_default.name.clone(),
        data: old_default_diff,
    }
    .insert(conn)?;

    let new_default_diff = snapshot::value_diff(non_defaulted_data, defaulted_data).unwrap();
    NewSnapshot {
        user_id: client.id,
        operation: ResourceOperation::Modified,
        resource_type: ResourceType::TagCategory,
        resource_id: new_default.name.clone(),
        data: new_default_diff,
    }
    .insert(conn)?;
    Ok(())
}

pub fn modification_snapshot(
    conn: &mut PgConnection,
    client: Client,
    old: &TagCategory,
    new: &TagCategory,
) -> QueryResult<()> {
    assert_eq!(old.id, new.id);

    let old_data = snapshot_data(old);
    let new_data = snapshot_data(new);
    if let Some(data) = snapshot::value_diff(old_data, new_data) {
        NewSnapshot {
            user_id: client.id,
            operation: ResourceOperation::Modified,
            resource_type: ResourceType::TagCategory,
            resource_id: old.name.clone(),
            data,
        }
        .insert(conn)?;
    }
    Ok(())
}

pub fn deletion_snapshot(conn: &mut PgConnection, client: Client, tag_category: &TagCategory) -> QueryResult<()> {
    unary_snapshot(conn, client, tag_category, ResourceOperation::Deleted)
}

fn snapshot_data(tag_category: &TagCategory) -> Value {
    json!({
        "name": tag_category.name,
        "color": tag_category.color,
        "default": tag_category.id == 0,
    })
}

fn unary_snapshot(
    conn: &mut PgConnection,
    client: Client,
    tag_category: &TagCategory,
    operation: ResourceOperation,
) -> QueryResult<()> {
    NewSnapshot {
        user_id: client.id,
        operation,
        resource_type: ResourceType::TagCategory,
        resource_id: tag_category.name.clone(),
        data: snapshot_data(tag_category),
    }
    .insert(conn)
}
