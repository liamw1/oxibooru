use crate::auth::Client;
use crate::model::enums::{ResourceOperation, ResourceType};
use crate::model::snapshot::NewSnapshot;
use crate::model::tag_category::TagCategory;
use crate::snapshot;
use serde_json::{Value, json};

pub fn creation_snapshot(client: Client, tag_category: &TagCategory) -> NewSnapshot {
    unary_snapshot(client, tag_category, ResourceOperation::Created)
}

pub fn modification_snapshot(client: Client, old: &TagCategory, new: &TagCategory) -> NewSnapshot {
    assert_eq!(old.id, new.id);

    let old_data = snapshot_data(old);
    let new_data = snapshot_data(new);
    let data = snapshot::value_diff(&old_data, &new_data).unwrap_or_default();
    NewSnapshot {
        user_id: client.id,
        operation: ResourceOperation::Modified,
        resource_type: ResourceType::TagCategory,
        resource_id: old.name.clone(),
        data,
    }
}

pub fn deletion_snapshot(client: Client, tag_category: &TagCategory) -> NewSnapshot {
    unary_snapshot(client, tag_category, ResourceOperation::Deleted)
}

fn snapshot_data(tag_category: &TagCategory) -> Value {
    json!({
        "name": tag_category.name,
        "color": tag_category.color,
        "default": tag_category.id == 0,
    })
}

fn unary_snapshot(client: Client, tag_category: &TagCategory, operation: ResourceOperation) -> NewSnapshot {
    NewSnapshot {
        user_id: client.id,
        operation,
        resource_type: ResourceType::TagCategory,
        resource_id: tag_category.name.clone(),
        data: snapshot_data(tag_category),
    }
}
