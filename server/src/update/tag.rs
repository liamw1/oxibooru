use crate::api::ApiResult;
use crate::config::RegexType;
use crate::model::tag::{NewTag, NewTagName};
use crate::model::user::User;
use crate::schema::{tag, tag_implication, tag_name};
use crate::{api, config};
use diesel::prelude::*;
use std::collections::HashSet;

/*
    Returns all tag ids implied from the given set of names.
    Returned ids will be distinct.
*/
pub fn get_or_create_tag_ids(
    conn: &mut PgConnection,
    client: Option<&User>,
    names: Vec<String>,
) -> ApiResult<Vec<i32>> {
    let mut implied_ids: Vec<i32> = tag_name::table
        .select(tag_name::tag_id)
        .filter(tag_name::name.eq_any(&names))
        .load(conn)?;
    let mut all_implied_tag_ids: HashSet<i32> = implied_ids.iter().copied().collect();

    let mut previous_len = 0;
    while all_implied_tag_ids.len() != previous_len {
        previous_len = all_implied_tag_ids.len();
        implied_ids = tag_implication::table
            .select(tag_implication::child_id)
            .filter(tag_implication::parent_id.eq_any(&implied_ids))
            .load(conn)?;
        all_implied_tag_ids.extend(implied_ids.iter().copied());
    }
    if !implied_ids.is_empty() {
        println!("WARNING: Tag implication cycle detected");
    }

    let existing_names: HashSet<String> = tag_name::table
        .select(tag_name::name)
        .filter(tag_name::tag_id.eq_any(&all_implied_tag_ids))
        .load(conn)?
        .into_iter()
        .collect();
    let mut tag_ids: Vec<_> = all_implied_tag_ids.into_iter().collect();

    let new_tag_names: Vec<_> = names
        .into_iter()
        .filter(|name| !existing_names.contains(name))
        .collect();
    new_tag_names
        .iter()
        .map(|name| api::verify_matches_regex(name, RegexType::Tag))
        .collect::<Result<_, _>>()?;

    // Create new tags if given unique names
    if !new_tag_names.is_empty() {
        api::verify_privilege(client, config::privileges().tag_create)?;

        let new_tag_ids: Vec<i32> = diesel::insert_into(tag::table)
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
        tag_ids.extend(new_tag_ids.into_iter());
    }
    Ok(tag_ids)
}
