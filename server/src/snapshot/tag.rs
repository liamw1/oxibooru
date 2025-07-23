use crate::auth::Client;
use crate::model::enums::{ResourceOperation, ResourceType};
use crate::model::snapshot::NewSnapshot;
use crate::snapshot;
use crate::string::SmallString;
use serde::Serialize;
use serde_json::json;

#[derive(Serialize)]
pub struct SnapshotData {
    category: SmallString,
    names: Vec<SmallString>,
    implications: Vec<SmallString>,
    suggestions: Vec<SmallString>,
}

pub fn creation_snapshot(client: Client, tag_data: SnapshotData) -> serde_json::Result<NewSnapshot> {
    unary_snapshot(client, tag_data, ResourceOperation::Created)
}

pub fn merge_snapshot(client: Client, absorbed_tag: SmallString, merge_to_tag: SmallString) -> NewSnapshot {
    let data = json!({
        "type": ResourceType::Tag,
        "id": merge_to_tag,
    });
    NewSnapshot {
        user_id: client.id,
        operation: ResourceOperation::Merged,
        resource_type: ResourceType::Tag,
        resource_id: absorbed_tag,
        data,
    }
}

pub fn modification_snapshot(client: Client, old: SnapshotData, new: SnapshotData) -> serde_json::Result<NewSnapshot> {
    let resource_id = old.names.first().unwrap().clone();

    let old_data = serde_json::to_value(old)?;
    let new_data = serde_json::to_value(new)?;
    let data = snapshot::value_diff(old_data, new_data).unwrap_or_default();
    Ok(NewSnapshot {
        user_id: client.id,
        operation: ResourceOperation::Modified,
        resource_type: ResourceType::Tag,
        resource_id,
        data,
    })
}

pub fn deletion_snapshot(client: Client, tag_data: SnapshotData) -> serde_json::Result<NewSnapshot> {
    unary_snapshot(client, tag_data, ResourceOperation::Deleted)
}

fn unary_snapshot(
    client: Client,
    tag_data: SnapshotData,
    operation: ResourceOperation,
) -> serde_json::Result<NewSnapshot> {
    let resource_id = tag_data.names.first().unwrap().clone();
    serde_json::to_value(tag_data).map(|data| NewSnapshot {
        user_id: client.id,
        operation,
        resource_type: ResourceType::Tag,
        resource_id,
        data,
    })
}
