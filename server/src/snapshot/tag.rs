use crate::api::ApiResult;
use crate::auth::Client;
use crate::model::enums::{ResourceOperation, ResourceType};
use crate::model::snapshot::NewSnapshot;
use crate::model::tag::TagName;
use crate::schema::{tag_category, tag_implication, tag_name, tag_suggestion};
use crate::string::SmallString;
use crate::{api, snapshot};
use diesel::prelude::*;
use serde::Serialize;
use serde_json::json;

#[derive(Clone, Serialize)]
pub struct SnapshotData {
    pub category: SmallString,
    pub names: Vec<SmallString>,
    pub implications: Vec<SmallString>,
    pub suggestions: Vec<SmallString>,
}

impl SnapshotData {
    pub fn retrieve(conn: &mut PgConnection, tag_id: i64, category_id: i64) -> QueryResult<Self> {
        let category = tag_category::table
            .find(category_id)
            .select(tag_category::name)
            .first(conn)?;
        let names = tag_name::table
            .select(tag_name::name)
            .filter(tag_name::tag_id.eq(tag_id))
            .load(conn)?;
        let implications = tag_name::table
            .inner_join(tag_implication::table.on(tag_name::tag_id.eq(tag_implication::child_id)))
            .select(tag_name::name)
            .filter(tag_implication::parent_id.eq(tag_id))
            .filter(TagName::primary())
            .load(conn)?;
        let suggestions = tag_name::table
            .inner_join(tag_suggestion::table.on(tag_name::tag_id.eq(tag_suggestion::child_id)))
            .select(tag_name::name)
            .filter(tag_suggestion::parent_id.eq(tag_id))
            .filter(TagName::primary())
            .load(conn)?;
        Ok(Self {
            category,
            names,
            implications,
            suggestions,
        })
    }

    fn sort_fields(&mut self) {
        self.names.sort_unstable();
        self.implications.sort_unstable();
        self.suggestions.sort_unstable();
    }
}

pub fn creation_snapshot(conn: &mut PgConnection, client: Client, tag_data: SnapshotData) -> ApiResult<()> {
    unary_snapshot(conn, client, tag_data, ResourceOperation::Created)
}

pub fn merge_snapshot(
    conn: &mut PgConnection,
    client: Client,
    absorbed_tag: SmallString,
    merge_to_tag: SmallString,
) -> QueryResult<()> {
    let data = json!([ResourceType::Tag, merge_to_tag]);
    NewSnapshot {
        user_id: client.id,
        operation: ResourceOperation::Merged,
        resource_type: ResourceType::Tag,
        resource_id: absorbed_tag,
        data,
    }
    .insert(conn)
}

pub fn modification_snapshot(
    conn: &mut PgConnection,
    client: Client,
    mut old: SnapshotData,
    mut new: SnapshotData,
) -> ApiResult<()> {
    let resource_id = old.names.first().unwrap().clone();

    old.sort_fields();
    new.sort_fields();
    let old_data = serde_json::to_value(old)?;
    let new_data = serde_json::to_value(new)?;
    if let Some(data) = snapshot::value_diff(old_data, new_data) {
        NewSnapshot {
            user_id: client.id,
            operation: ResourceOperation::Modified,
            resource_type: ResourceType::Tag,
            resource_id,
            data,
        }
        .insert(conn)?;
    }
    Ok(())
}

pub fn deletion_snapshot(conn: &mut PgConnection, client: Client, tag_data: SnapshotData) -> ApiResult<()> {
    unary_snapshot(conn, client, tag_data, ResourceOperation::Deleted)
}

fn unary_snapshot(
    conn: &mut PgConnection,
    client: Client,
    mut tag_data: SnapshotData,
    operation: ResourceOperation,
) -> ApiResult<()> {
    tag_data.sort_fields();
    let resource_id = tag_data.names.first().unwrap().clone();
    serde_json::to_value(tag_data)
        .map_err(api::Error::from)
        .and_then(|data| {
            NewSnapshot {
                user_id: client.id,
                operation,
                resource_type: ResourceType::Tag,
                resource_id,
                data,
            }
            .insert(conn)
            .map_err(api::Error::from)
        })
}
