use crate::model::pool::PoolName;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicroPool {
    pub id: i32,
    pub names: Vec<PoolName>,
    pub category: String,
    pub description: String,
    pub post_count: i64,
}
