use crate::model::post::PostTag;
use crate::model::tag::{Tag, TagImplication, TagName, TagSuggestion};
use crate::resource;
use crate::schema::{post_tag, tag, tag_category, tag_implication, tag_name, tag_suggestion};
use crate::util::DateTime;
use diesel::dsl::*;
use diesel::prelude::*;
use serde::Serialize;
use serde_with::skip_serializing_none;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use strum::{EnumString, EnumTable};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicroTag {
    pub names: Vec<String>,
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
            .map(Self::from_str)
            .collect::<Result<Vec<_>, _>>()?;
        for field in fields.into_iter() {
            table[field] = true;
        }
        Ok(table)
    }
}

// TODO: Remove renames by changing references to these names in client
#[skip_serializing_none]
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
    pub fn new_from_id(conn: &mut PgConnection, tag_id: i32, fields: &FieldTable<bool>) -> QueryResult<Self> {
        let mut tag_info = Self::new_batch_from_ids(conn, vec![tag_id], fields)?;
        assert_eq!(tag_info.len(), 1);
        Ok(tag_info.pop().unwrap())
    }

    pub fn new_batch(conn: &mut PgConnection, tags: Vec<Tag>, fields: &FieldTable<bool>) -> QueryResult<Vec<Self>> {
        let batch_size = tags.len();

        let mut categories = fields[Field::Category]
            .then_some(get_categories(conn, &tags)?)
            .unwrap_or_default();
        resource::check_batch_results(categories.len(), batch_size);

        let mut names = fields[Field::Names]
            .then_some(get_names(conn, &tags)?)
            .unwrap_or_default();
        resource::check_batch_results(names.len(), batch_size);

        let mut implications = fields[Field::Implications]
            .then_some(get_implications(conn, &tags)?)
            .unwrap_or_default();
        resource::check_batch_results(implications.len(), batch_size);

        let mut suggestions = fields[Field::Suggestions]
            .then_some(get_suggestions(conn, &tags)?)
            .unwrap_or_default();
        resource::check_batch_results(suggestions.len(), batch_size);

        let mut usages = fields[Field::Usages]
            .then_some(get_usages(conn, &tags)?)
            .unwrap_or_default();
        resource::check_batch_results(usages.len(), batch_size);

        let results = tags
            .into_iter()
            .rev()
            .map(|tag| Self {
                version: fields[Field::Version].then_some(tag.last_edit_time),
                description: fields[Field::Description].then_some(tag.description),
                creation_time: fields[Field::CreationTime].then_some(tag.creation_time),
                last_edit_time: fields[Field::LastEditTime].then_some(tag.last_edit_time),
                category: categories.pop(),
                names: names.pop(),
                implications: implications.pop(),
                suggestions: suggestions.pop(),
                usages: usages.pop(),
            })
            .collect::<Vec<_>>();
        Ok(results.into_iter().rev().collect())
    }

    pub fn new_batch_from_ids(
        conn: &mut PgConnection,
        tag_ids: Vec<i32>,
        fields: &FieldTable<bool>,
    ) -> QueryResult<Vec<Self>> {
        let unordered_tags = tag::table.filter(tag::id.eq_any(&tag_ids)).load(conn)?;
        let tags = resource::order_by(unordered_tags, &tag_ids);
        Self::new_batch(conn, tags, fields)
    }
}

fn get_categories(conn: &mut PgConnection, tags: &[Tag]) -> QueryResult<Vec<String>> {
    let categories: Vec<_> = tags.iter().map(|tag| tag.category_id).collect();
    let category_names: HashMap<i32, String> = tag_category::table
        .select((tag_category::id, tag_category::name))
        .filter(tag_category::id.eq_any(categories))
        .load(conn)?
        .into_iter()
        .collect();
    Ok(tags
        .iter()
        .map(|tag| category_names[&tag.category_id].clone())
        .collect())
}

fn get_names(conn: &mut PgConnection, tags: &[Tag]) -> QueryResult<Vec<Vec<String>>> {
    Ok(TagName::belonging_to(tags)
        .order(tag_name::order)
        .load::<TagName>(conn)?
        .grouped_by(tags)
        .into_iter()
        .map(|tag_names| tag_names.into_iter().map(|tag_name| tag_name.name).collect())
        .collect())
}

fn get_implications(conn: &mut PgConnection, tags: &[Tag]) -> QueryResult<Vec<Vec<MicroTag>>> {
    let implications: Vec<(TagImplication, i32, String)> = TagImplication::belonging_to(tags)
        .inner_join(tag::table.on(tag::id.eq(tag_implication::child_id)))
        .inner_join(tag_name::table.on(tag_name::tag_id.eq(tag_implication::child_id)))
        .select((TagImplication::as_select(), tag::category_id, tag_name::name))
        .order((tag_name::order, tag_name::name))
        .load(conn)?;
    let all_implied_tag_ids: HashSet<i32> = implications
        .iter()
        .map(|(implication, ..)| implication.child_id)
        .collect();

    let implication_usages: HashMap<i32, i64> = post_tag::table
        .group_by(post_tag::tag_id)
        .select((post_tag::tag_id, count(post_tag::tag_id)))
        .filter(post_tag::tag_id.eq_any(&all_implied_tag_ids))
        .load(conn)?
        .into_iter()
        .collect();
    let category_names: HashMap<i32, String> = tag_category::table
        .select((tag_category::id, tag_category::name))
        .load(conn)?
        .into_iter()
        .collect();

    Ok(implications
        .grouped_by(tags)
        .into_iter()
        .map(|implications_on_tag| {
            resource::collect_tag_data(implications_on_tag, |implication| implication.child_id)
                .into_iter()
                .map(|tag| MicroTag {
                    names: tag.names,
                    category: category_names[&tag.category_id].clone(),
                    usages: implication_usages.get(&tag.id).copied().unwrap_or(0),
                })
                .collect()
        })
        .collect())
}

fn get_suggestions(conn: &mut PgConnection, tags: &[Tag]) -> QueryResult<Vec<Vec<MicroTag>>> {
    let suggestions: Vec<(TagSuggestion, i32, String)> = TagSuggestion::belonging_to(tags)
        .inner_join(tag::table.on(tag::id.eq(tag_suggestion::child_id)))
        .inner_join(tag_name::table.on(tag_name::tag_id.eq(tag_suggestion::child_id)))
        .select((TagSuggestion::as_select(), tag::category_id, tag_name::name))
        .order((tag_name::order, tag_name::name))
        .load(conn)?;
    let all_suggested_tag_ids: HashSet<i32> = suggestions.iter().map(|(suggestion, ..)| suggestion.child_id).collect();

    let suggestion_usages: HashMap<i32, i64> = post_tag::table
        .group_by(post_tag::tag_id)
        .select((post_tag::tag_id, count(post_tag::tag_id)))
        .filter(post_tag::tag_id.eq_any(all_suggested_tag_ids))
        .load(conn)?
        .into_iter()
        .collect();
    let category_names: HashMap<i32, String> = tag_category::table
        .select((tag_category::id, tag_category::name))
        .load(conn)?
        .into_iter()
        .collect();

    Ok(suggestions
        .grouped_by(tags)
        .into_iter()
        .map(|suggestions_on_tag| {
            resource::collect_tag_data(suggestions_on_tag, |suggestion| suggestion.child_id)
                .into_iter()
                .map(|tag| MicroTag {
                    names: tag.names,
                    category: category_names[&tag.category_id].clone(),
                    usages: suggestion_usages.get(&tag.id).copied().unwrap_or(0),
                })
                .collect()
        })
        .collect())
}

fn get_usages(conn: &mut PgConnection, tags: &[Tag]) -> QueryResult<Vec<i64>> {
    PostTag::belonging_to(tags)
        .group_by(post_tag::tag_id)
        .select((post_tag::tag_id, count(post_tag::tag_id)))
        .load(conn)
        .map(|usages| {
            resource::order_as(usages, tags, |(id, _)| *id)
                .into_iter()
                .map(|post_count| post_count.map(|(_, count)| count).unwrap_or(0))
                .collect()
        })
}
