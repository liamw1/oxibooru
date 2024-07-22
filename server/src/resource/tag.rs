use crate::model::tag::{Tag, TagName};
use crate::util::DateTime;
use diesel::prelude::*;
use serde::Serialize;
use std::str::FromStr;
use strum::{EnumString, EnumTable};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicroTag {
    pub names: Vec<TagName>,
    pub category: String,
    pub usages: i64,
}

#[derive(Clone, Copy, EnumString, EnumTable)]
#[strum(serialize_all = "camelCase")]
pub enum Field {
    Version,
    Description,
    CreationTime,
    LastEditTime,
    Category,
    Names,
    Implications,
    Suggestions,
    Usages,
}

impl Field {
    pub fn create_table(fields_str: &str) -> Result<FieldTable<bool>, <Self as FromStr>::Err> {
        let mut table = FieldTable::filled(false);
        let fields = fields_str
            .split(',')
            .into_iter()
            .map(Self::from_str)
            .collect::<Result<Vec<_>, _>>()?;
        for field in fields.into_iter() {
            table[field] = true;
        }
        Ok(table)
    }
}

// TODO: Remove renames by changing references to these names in client
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagInfo {
    version: Option<DateTime>,
    description: Option<String>,
    creation_time: Option<DateTime>,
    last_edit_time: Option<DateTime>,
    category: Option<String>,
    names: Option<Vec<String>>,
    implications: Option<Vec<MicroTag>>,
    suggestions: Option<Vec<MicroTag>>,
    usages: Option<i64>,
}

impl TagInfo {
    pub fn new(conn: &mut PgConnection, tag: Tag, fields: &FieldTable<bool>) -> QueryResult<Self> {
        let mut tag_info = Self::new_batch(conn, vec![tag], fields)?;
        assert_eq!(tag_info.len(), 1);
        Ok(tag_info.pop().unwrap())
    }

    pub fn new_batch(conn: &mut PgConnection, mut tags: Vec<Tag>, fields: &FieldTable<bool>) -> QueryResult<Vec<Self>> {
        let batch_size = tags.len();

        unimplemented!()
    }
}
