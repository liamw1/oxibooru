use crate::api::ApiResult;
use crate::auth::header::Client;
use crate::config::RegexType;
use crate::model::enums::ResourceType;
use crate::model::tag::{NewTag, NewTagName, TagImplication, TagSuggestion};
use crate::schema::{tag, tag_implication, tag_name, tag_suggestion};
use crate::string::SmallString;
use crate::time::DateTime;
use crate::{api, config};
use diesel::prelude::*;
use std::collections::HashSet;

/// Updates last_edit_time of tag with given `tag_id`.
pub fn last_edit_time(conn: &mut PgConnection, tag_id: i64) -> ApiResult<()> {
    diesel::update(tag::table.find(tag_id))
        .set(tag::last_edit_time.eq(DateTime::now()))
        .execute(conn)?;
    Ok(())
}

/// Appends `names` onto the current list of names for the tag with id `tag_id`.
pub fn add_names(
    conn: &mut PgConnection,
    tag_id: i64,
    current_name_count: i32,
    names: Vec<SmallString>,
) -> ApiResult<()> {
    names
        .iter()
        .try_for_each(|name| api::verify_matches_regex(name, RegexType::Tag))?;

    let new_names: Vec<_> = names
        .iter()
        .enumerate()
        .map(|(i, name)| (current_name_count + i as i32, name))
        .map(|(order, name)| NewTagName { tag_id, order, name })
        .collect();
    diesel::insert_into(tag_name::table).values(new_names).execute(conn)?;
    Ok(())
}

/// Deletes all names for the tag with id `tag_id`.
/// Returns number of names deleted.
pub fn delete_names(conn: &mut PgConnection, tag_id: i64) -> QueryResult<usize> {
    diesel::delete(tag_name::table)
        .filter(tag_name::tag_id.eq(tag_id))
        .execute(conn)
}

/// Adds `implied_ids` to the list of implications for the tag with id `tag_id`.
pub fn add_implications(conn: &mut PgConnection, tag_id: i64, implied_ids: Vec<i64>) -> ApiResult<()> {
    let new_implications: Vec<_> = implied_ids
        .into_iter()
        .map(|child_id| {
            (tag_id != child_id)
                .then_some(TagImplication {
                    parent_id: tag_id,
                    child_id,
                })
                .ok_or(api::Error::CyclicDependency(ResourceType::TagImplication))
        })
        .collect::<Result<_, _>>()?;
    diesel::insert_into(tag_implication::table)
        .values(new_implications)
        .execute(conn)?;
    Ok(())
}

/// Adds `suggested_ids` to the list of suggestions for the tag with id `tag_id`.
pub fn add_suggestions(conn: &mut PgConnection, tag_id: i64, suggested_ids: Vec<i64>) -> ApiResult<()> {
    let new_suggestions: Vec<_> = suggested_ids
        .into_iter()
        .map(|child_id| {
            (tag_id != child_id)
                .then_some(TagSuggestion {
                    parent_id: tag_id,
                    child_id,
                })
                .ok_or(api::Error::CyclicDependency(ResourceType::TagSuggestion))
        })
        .collect::<Result<_, _>>()?;
    diesel::insert_into(tag_suggestion::table)
        .values(new_suggestions)
        .execute(conn)?;
    Ok(())
}

/// Returns all tag ids implied from the given set of names.
/// Returned ids will be distinct.
///
/// Requires tag creation privileges if new names are given.
/// Checks that each new name matches on the Tag regex.
pub fn get_or_create_tag_ids(
    conn: &mut PgConnection,
    client: Client,
    names: &[SmallString],
    detect_cyclic_dependencies: bool,
) -> ApiResult<Vec<i64>> {
    let mut implied_ids: Vec<i64> = tag_name::table
        .select(tag_name::tag_id)
        .filter(tag_name::name.eq_any(names))
        .distinct()
        .load(conn)?;
    let mut all_implied_tag_ids: HashSet<i64> = implied_ids.iter().copied().collect();

    let mut iteration = 0;
    let mut previous_len = 0;
    while all_implied_tag_ids.len() != previous_len {
        iteration += 1;
        previous_len = all_implied_tag_ids.len();
        implied_ids = tag_implication::table
            .select(tag_implication::child_id)
            .filter(tag_implication::parent_id.eq_any(&implied_ids))
            .distinct()
            .load(conn)?;
        all_implied_tag_ids.extend(implied_ids.iter().copied());
    }
    if detect_cyclic_dependencies && !implied_ids.is_empty() && iteration > 1 {
        return Err(api::Error::CyclicDependency(ResourceType::TagImplication));
    }

    let mut tag_ids: Vec<_> = all_implied_tag_ids.into_iter().collect();
    let existing_names: HashSet<String> = tag_name::table
        .select(tag_name::name)
        .filter(tag_name::tag_id.eq_any(&tag_ids))
        .load(conn)?
        .into_iter()
        .map(|name: SmallString| name.to_lowercase())
        .collect();

    let new_tag_names: Vec<_> = names
        .iter()
        .filter(|name| !existing_names.contains(&name.to_lowercase()))
        .collect();
    new_tag_names
        .iter()
        .try_for_each(|name| api::verify_matches_regex(name, RegexType::Tag))?;

    // Create new tags if given unique names
    if !new_tag_names.is_empty() {
        api::verify_privilege(client, config::privileges().tag_create)?;

        let new_tag_ids: Vec<i64> = diesel::insert_into(tag::table)
            .values(vec![NewTag::default(); new_tag_names.len()])
            .returning(tag::id)
            .get_results(conn)?;
        let new_tag_names: Vec<_> = new_tag_ids
            .iter()
            .zip(new_tag_names.iter())
            .map(|(&tag_id, name)| NewTagName { tag_id, order: 0, name })
            .collect();
        diesel::insert_into(tag_name::table)
            .values(new_tag_names)
            .execute(conn)?;
        tag_ids.extend(new_tag_ids);
    }
    Ok(tag_ids)
}
