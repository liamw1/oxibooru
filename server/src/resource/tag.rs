use crate::model::tag::{Tag, TagImplication, TagName, TagSuggestion};
use crate::resource::{self, BoolFill};
use crate::schema::{tag, tag_category, tag_implication, tag_name, tag_statistics, tag_suggestion};
use crate::string::{LargeString, SmallString};
use crate::time::DateTime;
use diesel::prelude::*;
use serde::Serialize;
use serde_with::skip_serializing_none;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use strum::{EnumString, EnumTable};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicroTag {
    pub names: Rc<[SmallString]>,
    pub category: SmallString,
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

impl BoolFill for FieldTable<bool> {
    fn filled(val: bool) -> Self {
        Self::filled(val)
    }
}

#[skip_serializing_none]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagInfo {
    version: Option<DateTime>,
    description: Option<LargeString>,
    creation_time: Option<DateTime>,
    last_edit_time: Option<DateTime>,
    category: Option<SmallString>,
    names: Option<Vec<SmallString>>,
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

    pub fn new_from_id(conn: &mut PgConnection, tag_id: i64, fields: &FieldTable<bool>) -> QueryResult<Self> {
        let mut tag_info = Self::new_batch_from_ids(conn, &[tag_id], fields)?;
        assert_eq!(tag_info.len(), 1);
        Ok(tag_info.pop().unwrap())
    }

    pub fn new_batch(conn: &mut PgConnection, tags: Vec<Tag>, fields: &FieldTable<bool>) -> QueryResult<Vec<Self>> {
        let mut categories = resource::retrieve(fields[Field::Category], || get_categories(conn, &tags))?;
        let mut names = resource::retrieve(fields[Field::Names], || get_names(conn, &tags))?;
        let mut implications = resource::retrieve(fields[Field::Implications], || get_implications(conn, &tags))?;
        let mut suggestions = resource::retrieve(fields[Field::Suggestions], || get_suggestions(conn, &tags))?;
        let mut usages = resource::retrieve(fields[Field::Usages], || get_usages(conn, &tags))?;

        let batch_size = tags.len();
        resource::check_batch_results(categories.len(), batch_size);
        resource::check_batch_results(names.len(), batch_size);
        resource::check_batch_results(implications.len(), batch_size);
        resource::check_batch_results(suggestions.len(), batch_size);
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
        tag_ids: &[i64],
        fields: &FieldTable<bool>,
    ) -> QueryResult<Vec<Self>> {
        let unordered_tags = tag::table.filter(tag::id.eq_any(tag_ids)).load(conn)?;
        let tags = resource::order_as(unordered_tags, tag_ids);
        Self::new_batch(conn, tags, fields)
    }
}

fn get_categories(conn: &mut PgConnection, tags: &[Tag]) -> QueryResult<Vec<SmallString>> {
    let tag_ids: Vec<_> = tags.iter().map(|tag| tag.id).collect();
    tag::table
        .inner_join(tag_category::table)
        .select((tag::id, tag_category::name))
        .filter(tag::id.eq_any(&tag_ids))
        .load(conn)
        .map(|category_names| {
            resource::order_transformed_as(category_names, &tag_ids, |&(tag_id, _)| tag_id)
                .into_iter()
                .map(|(_, category_name)| category_name)
                .collect()
        })
}

fn get_names(conn: &mut PgConnection, tags: &[Tag]) -> QueryResult<Vec<Vec<SmallString>>> {
    Ok(TagName::belonging_to(tags)
        .order(tag_name::order)
        .load::<TagName>(conn)?
        .grouped_by(tags)
        .into_iter()
        .map(|tag_names| tag_names.into_iter().map(|tag_name| tag_name.name).collect())
        .collect())
}

fn get_implications(conn: &mut PgConnection, tags: &[Tag]) -> QueryResult<Vec<Vec<MicroTag>>> {
    let implication_info = tag::table.inner_join(tag_statistics::table).inner_join(tag_name::table);
    let implications: Vec<(TagImplication, i64, i64)> = TagImplication::belonging_to(tags)
        .inner_join(implication_info.on(tag::id.eq(tag_implication::child_id)))
        .select((TagImplication::as_select(), tag::category_id, tag_statistics::usage_count))
        .filter(TagName::primary())
        .order_by(tag_name::name)
        .load(conn)?;
    let implication_ids: HashSet<i64> = implications
        .iter()
        .map(|(implication, ..)| implication.child_id)
        .collect();

    let implication_names: Vec<(i64, SmallString)> = tag_name::table
        .select((tag_name::tag_id, tag_name::name))
        .filter(tag_name::tag_id.eq_any(implication_ids))
        .order_by((tag_name::tag_id, tag_name::order))
        .load(conn)?;
    let names_map = resource::collect_names(implication_names);

    let category_names: HashMap<i64, SmallString> = tag_category::table
        .select((tag_category::id, tag_category::name))
        .load(conn)?
        .into_iter()
        .collect();

    Ok(implications
        .grouped_by(tags)
        .into_iter()
        .map(|implications_on_tag| {
            implications_on_tag
                .into_iter()
                .map(|(implication, category_id, usages)| MicroTag {
                    names: names_map[&implication.child_id].clone(),
                    category: category_names[&category_id].clone(),
                    usages,
                })
                .collect()
        })
        .collect())
}

fn get_suggestions(conn: &mut PgConnection, tags: &[Tag]) -> QueryResult<Vec<Vec<MicroTag>>> {
    let suggestion_info = tag::table.inner_join(tag_statistics::table).inner_join(tag_name::table);
    let suggestions: Vec<(TagSuggestion, i64, i64)> = TagSuggestion::belonging_to(tags)
        .inner_join(suggestion_info.on(tag::id.eq(tag_suggestion::child_id)))
        .select((TagSuggestion::as_select(), tag::category_id, tag_statistics::usage_count))
        .filter(TagName::primary())
        .order_by(tag_name::name)
        .load(conn)?;
    let suggestion_ids: HashSet<i64> = suggestions.iter().map(|(suggestion, ..)| suggestion.child_id).collect();

    let suggestion_names: Vec<(i64, SmallString)> = tag_name::table
        .select((tag_name::tag_id, tag_name::name))
        .filter(tag_name::tag_id.eq_any(suggestion_ids))
        .order_by((tag_name::tag_id, tag_name::order))
        .load(conn)?;
    let names_map = resource::collect_names(suggestion_names);

    let category_names: HashMap<i64, SmallString> = tag_category::table
        .select((tag_category::id, tag_category::name))
        .load(conn)?
        .into_iter()
        .collect();

    Ok(suggestions
        .grouped_by(tags)
        .into_iter()
        .map(|suggestions_on_tag| {
            suggestions_on_tag
                .into_iter()
                .map(|(suggestion, category_id, usages)| MicroTag {
                    names: names_map[&suggestion.child_id].clone(),
                    category: category_names[&category_id].clone(),
                    usages,
                })
                .collect()
        })
        .collect())
}

fn get_usages(conn: &mut PgConnection, tags: &[Tag]) -> QueryResult<Vec<i64>> {
    let tag_ids: Vec<_> = tags.iter().map(Identifiable::id).copied().collect();
    tag_statistics::table
        .select((tag_statistics::tag_id, tag_statistics::usage_count))
        .filter(tag_statistics::tag_id.eq_any(&tag_ids))
        .load(conn)
        .map(|tag_usages| {
            resource::order_transformed_as(tag_usages, &tag_ids, |&(id, _)| id)
                .into_iter()
                .map(|(_, usages)| usages)
                .collect()
        })
}
