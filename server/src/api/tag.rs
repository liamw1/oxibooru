use crate::api::{ApiResult, AuthResult, PagedQuery, PagedResponse, ResourceQuery};
use crate::model::tag::{NewTagImplication, NewTagName, NewTagSuggestion, Tag};
use crate::resource::tag::{FieldTable, TagInfo};
use crate::schema::{tag, tag_category, tag_implication, tag_name, tag_suggestion};
use crate::util::DateTime;
use crate::{api, config, resource, search, update};
use diesel::prelude::*;
use serde::Deserialize;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list_tags = warp::get()
        .and(warp::path!("tags"))
        .and(api::auth())
        .and(warp::query())
        .map(list_tags)
        .map(api::Reply::from);
    let get_tag = warp::get()
        .and(warp::path!("tag" / String))
        .and(api::auth())
        .and(api::resource_query())
        .map(get_tag)
        .map(api::Reply::from);
    let update_tag = warp::put()
        .and(warp::path!("tag" / String))
        .and(api::auth())
        .and(api::resource_query())
        .and(warp::body::json())
        .map(update_tag)
        .map(api::Reply::from);

    list_tags.or(get_tag).or(update_tag)
}

type PagedTagInfo = PagedResponse<TagInfo>;

const MAX_TAGS_PER_PAGE: i64 = 50;

fn create_field_table(fields: Option<&str>) -> Result<FieldTable<bool>, Box<dyn std::error::Error>> {
    fields
        .map(resource::tag::Field::create_table)
        .transpose()
        .map(|opt_table| opt_table.unwrap_or(FieldTable::filled(true)))
        .map_err(Box::from)
}

fn list_tags(auth: AuthResult, query: PagedQuery) -> ApiResult<PagedTagInfo> {
    let _timer = crate::util::Timer::new("list_tags");

    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().tag_list)?;

    let offset = query.offset.unwrap_or(0);
    let limit = std::cmp::min(query.limit, MAX_TAGS_PER_PAGE);
    let fields = create_field_table(query.fields())?;

    crate::establish_connection()?.transaction(|conn| {
        let mut search_criteria = search::tag::parse_search_criteria(query.criteria())?;
        search_criteria.add_offset_and_limit(offset, limit);
        let count_query = search::tag::build_query(&search_criteria)?;
        let sql_query = search::tag::build_query(&search_criteria)?;

        let total = count_query.count().first(conn)?;
        let selected_tags: Vec<i32> = search::tag::get_ordered_ids(conn, sql_query, &search_criteria)?;
        Ok(PagedTagInfo {
            query: query.query.query,
            offset,
            limit,
            total,
            results: TagInfo::new_batch_from_ids(conn, selected_tags, &fields)?,
        })
    })
}

fn get_tag(name: String, auth: AuthResult, query: ResourceQuery) -> ApiResult<TagInfo> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().tag_view)?;

    let fields = create_field_table(query.fields())?;
    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    crate::establish_connection()?.transaction(|conn| {
        let tag_id = tag_name::table
            .select(tag_name::tag_id)
            .filter(tag_name::name.eq(name))
            .first(conn)?;
        TagInfo::new_from_id(conn, tag_id, &fields).map_err(api::Error::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TagUpdateInfo {
    version: DateTime,
    category: Option<String>,
    description: Option<String>,
    names: Option<Vec<String>>,
    implications: Option<Vec<String>>,
    suggestions: Option<Vec<String>>,
}

fn update_tag(name: String, auth: AuthResult, query: ResourceQuery, update: TagUpdateInfo) -> ApiResult<TagInfo> {
    let _timer = crate::util::Timer::new("update_tag");

    let client = auth?;
    let fields = create_field_table(query.fields())?;
    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    crate::establish_connection()?.transaction(|conn| {
        let tag = Tag::from_name(conn, &name)?;
        api::verify_version(tag.last_edit_time, update.version)?;

        // Update category
        if let Some(category) = update.category {
            api::verify_privilege(client.as_ref(), config::privileges().tag_edit_category)?;

            let category_id: i32 = tag_category::table
                .select(tag_category::id)
                .filter(tag_category::name.eq(category))
                .first(conn)?;
            diesel::update(tag::table.find(tag.id))
                .set(tag::category_id.eq(category_id))
                .execute(conn)?;
        }

        // Update description
        if let Some(description) = update.description {
            api::verify_privilege(client.as_ref(), config::privileges().tag_edit_description)?;

            diesel::update(tag::table.find(tag.id))
                .set(tag::description.eq(description))
                .execute(conn)?;
        }

        // Update names
        if let Some(names) = update.names {
            api::verify_privilege(client.as_ref(), config::privileges().tag_edit_name)?;

            diesel::delete(tag_name::table)
                .filter(tag_name::tag_id.eq(tag.id))
                .execute(conn)?;

            let tag_id = tag.id;
            let updated_names: Vec<_> = names
                .iter()
                .enumerate()
                .map(|(order, name)| (order as i32, name))
                .map(|(order, name)| NewTagName { tag_id, order, name })
                .collect();
            diesel::insert_into(tag_name::table)
                .values(updated_names)
                .execute(conn)?;
        }

        // Update Implications
        if let Some(implications) = update.implications {
            api::verify_privilege(client.as_ref(), config::privileges().tag_edit_implication)?;

            diesel::delete(tag_implication::table)
                .filter(tag_implication::parent_id.eq(tag.id))
                .execute(conn)?;

            let updated_implication_ids = update::tag::get_or_create_tag_ids(conn, client.as_ref(), implications)?;
            let updated_implications: Vec<_> = updated_implication_ids
                .into_iter()
                .map(|child_id| NewTagImplication {
                    parent_id: tag.id,
                    child_id,
                })
                .collect();
            diesel::insert_into(tag_implication::table)
                .values(updated_implications)
                .execute(conn)?;
        }

        // Update Suggestions
        if let Some(suggestions) = update.suggestions {
            api::verify_privilege(client.as_ref(), config::privileges().tag_edit_suggestion)?;

            diesel::delete(tag_suggestion::table)
                .filter(tag_suggestion::parent_id.eq(tag.id))
                .execute(conn)?;

            let updated_suggestion_ids = update::tag::get_or_create_tag_ids(conn, client.as_ref(), suggestions)?;
            let updated_suggestions: Vec<_> = updated_suggestion_ids
                .into_iter()
                .map(|child_id| NewTagSuggestion {
                    parent_id: tag.id,
                    child_id,
                })
                .collect();
            diesel::insert_into(tag_suggestion::table)
                .values(updated_suggestions)
                .execute(conn)?;
        }

        TagInfo::new_from_id(conn, tag.id, &fields).map_err(api::Error::from)
    })
}
