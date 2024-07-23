use crate::model::post::PostTag;
use crate::model::tag::{Tag, TagId, TagImplication, TagName, TagSuggestion};
use crate::resource;
use crate::schema::{post_tag, tag, tag_category, tag_implication, tag_suggestion};
use crate::util::DateTime;
use diesel::dsl::*;
use diesel::prelude::*;
use serde::Serialize;
use serde_with::skip_serializing_none;
use std::collections::HashMap;
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
    pub fn new(conn: &mut PgConnection, tag: Tag, fields: &FieldTable<bool>) -> QueryResult<Self> {
        let mut tag_info = Self::new_batch(conn, vec![tag], fields)?;
        assert_eq!(tag_info.len(), 1);
        Ok(tag_info.pop().unwrap())
    }

    pub fn new_from_id(conn: &mut PgConnection, tag_id: i32, fields: &FieldTable<bool>) -> QueryResult<Self> {
        let mut tag_info = Self::new_batch_from_ids(conn, vec![tag_id], fields)?;
        assert_eq!(tag_info.len(), 1);
        Ok(tag_info.pop().unwrap())
    }

    pub fn new_batch(conn: &mut PgConnection, mut tags: Vec<Tag>, fields: &FieldTable<bool>) -> QueryResult<Vec<Self>> {
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

        let mut results: Vec<Self> = Vec::new();
        while let Some(tag) = tags.pop() {
            results.push(Self {
                version: fields[Field::Version].then_some(tag.last_edit_time),
                description: fields[Field::Description].then_some(tag.description),
                creation_time: fields[Field::CreationTime].then_some(tag.creation_time),
                last_edit_time: fields[Field::LastEditTime].then_some(tag.last_edit_time),
                category: categories.pop(),
                names: names.pop(),
                implications: implications.pop(),
                suggestions: suggestions.pop(),
                usages: usages.pop(),
            });
        }
        Ok(results.into_iter().rev().collect())
    }

    pub fn new_batch_from_ids(
        conn: &mut PgConnection,
        tag_ids: Vec<i32>,
        fields: &FieldTable<bool>,
    ) -> QueryResult<Vec<Self>> {
        let tags = get_tags(conn, &tag_ids)?;
        Self::new_batch(conn, tags, fields)
    }
}

fn get_tags(conn: &mut PgConnection, tag_ids: &[i32]) -> QueryResult<Vec<Tag>> {
    // We get tags here, but this query doesn't preserve order
    let mut tags = tag::table
        .select(Tag::as_select())
        .filter(tag::id.eq_any(tag_ids))
        .load(conn)?;

    /*
        This algorithm is O(n^2) in tag_ids.len(), which could be made O(n) with a HashMap implementation.
        However, for small n this Vec-based implementation is probably much faster. Since we retrieve
        40-50 tags at a time, I'm leaving it like this for the time being until it proves to be slow.
    */
    let mut index = 0;
    while index < tag_ids.len() {
        let tag_id = tags[index].id;
        let correct_index = tag_ids.iter().position(|&id| id == tag_id).unwrap();
        if index != correct_index {
            tags.swap(index, correct_index);
        } else {
            index += 1;
        }
    }

    Ok(tags)
}

fn get_categories(conn: &mut PgConnection, tags: &[Tag]) -> QueryResult<Vec<String>> {
    let category_names: HashMap<i32, String> = tag_category::table
        .select((tag_category::id, tag_category::name))
        .load(conn)?
        .into_iter()
        .collect();
    Ok(tags.iter().map(|tag| category_names[&tag.id].clone()).collect())
}

fn get_names(conn: &mut PgConnection, tags: &[Tag]) -> QueryResult<Vec<Vec<String>>> {
    let names = TagName::belonging_to(tags).select(TagName::as_select()).load(conn)?;
    Ok(names
        .grouped_by(tags)
        .into_iter()
        .map(|tag_names| tag_names.into_iter().map(|tag_name| tag_name.name).collect())
        .collect())
}

fn get_implications(conn: &mut PgConnection, tags: &[Tag]) -> QueryResult<Vec<Vec<MicroTag>>> {
    let implications = TagImplication::belonging_to(tags)
        .inner_join(tag::table.on(tag::id.eq(tag_implication::child_id)))
        .select(Tag::as_select())
        .load(conn)?;
    let implication_usages: HashMap<i32, i64> = PostTag::belonging_to(&implications)
        .group_by(post_tag::tag_id)
        .select((post_tag::tag_id, count(post_tag::tag_id)))
        .load(conn)?
        .into_iter()
        .collect();
    let category_names: HashMap<i32, String> = tag_category::table
        .select((tag_category::id, tag_category::name))
        .load(conn)?
        .into_iter()
        .collect();

    let implication_names = TagName::belonging_to(&implications)
        .select(TagName::as_select())
        .load(conn)?;

    let process_tag = |tag_info: (&Tag, Vec<TagName>)| -> Option<MicroTag> {
        let (implication, tag_names) = tag_info;
        (!tag_names.is_empty()).then_some({
            let mut names: Vec<_> = tag_names.into_iter().collect();
            names.sort();
            MicroTag {
                names,
                category: category_names[&implication.category_id].clone(),
                usages: implication_usages.get(&implication.id).map(|x| *x).unwrap_or(0),
            }
        })
    };
    Ok(implication_names
        .grouped_by(tags)
        .into_iter()
        .map(|implications_on_tag| {
            implications
                .iter()
                .zip(implications_on_tag.grouped_by(&implications).into_iter())
                .filter_map(process_tag)
                .collect()
        })
        .collect())
}

fn get_suggestions(conn: &mut PgConnection, tags: &[Tag]) -> QueryResult<Vec<Vec<MicroTag>>> {
    let suggestions = TagSuggestion::belonging_to(tags)
        .inner_join(tag::table.on(tag::id.eq(tag_suggestion::child_id)))
        .select(Tag::as_select())
        .load(conn)?;
    let suggestion_usages: HashMap<i32, i64> = PostTag::belonging_to(&suggestions)
        .group_by(post_tag::tag_id)
        .select((post_tag::tag_id, count(post_tag::tag_id)))
        .load(conn)?
        .into_iter()
        .collect();
    let category_names: HashMap<i32, String> = tag_category::table
        .select((tag_category::id, tag_category::name))
        .load(conn)?
        .into_iter()
        .collect();

    let suggestion_names = TagName::belonging_to(&suggestions)
        .select(TagName::as_select())
        .load(conn)?;

    let process_tag = |tag_info: (&Tag, Vec<TagName>)| -> Option<MicroTag> {
        let (suggestion, tag_names) = tag_info;
        (!tag_names.is_empty()).then_some({
            let mut names: Vec<_> = tag_names.into_iter().collect();
            names.sort();
            MicroTag {
                names,
                category: category_names[&suggestion.category_id].clone(),
                usages: suggestion_usages.get(&suggestion.id).map(|x| *x).unwrap_or(0),
            }
        })
    };
    Ok(suggestion_names
        .grouped_by(tags)
        .into_iter()
        .map(|suggestions_on_tag| {
            suggestions
                .iter()
                .zip(suggestions_on_tag.grouped_by(&suggestions).into_iter())
                .filter_map(process_tag)
                .collect()
        })
        .collect())
}

fn get_usages(conn: &mut PgConnection, tags: &[Tag]) -> QueryResult<Vec<i64>> {
    let usages: Vec<(TagId, i64)> = PostTag::belonging_to(tags)
        .group_by(post_tag::tag_id)
        .select((post_tag::tag_id, count(post_tag::post_id)))
        .load(conn)?;
    Ok(usages
        .grouped_by(tags)
        .into_iter()
        .map(|counts| counts.first().map(|(_, count)| *count).unwrap_or(0))
        .collect())
}
