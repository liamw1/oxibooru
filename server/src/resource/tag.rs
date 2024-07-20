use crate::model::tag::TagName;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicroTag {
    pub names: Vec<TagName>,
    pub category: String,
    pub usages: i64,
}
