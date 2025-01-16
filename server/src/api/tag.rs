use crate::api::{ApiResult, AuthResult, DeleteRequest, MergeRequest, PagedQuery, PagedResponse, ResourceQuery};
use crate::model::enums::ResourceType;
use crate::model::post::PostTag;
use crate::model::tag::{NewTag, TagImplication, TagSuggestion};
use crate::resource::tag::{FieldTable, TagInfo};
use crate::schema::{database_statistics, post_tag, tag, tag_category, tag_implication, tag_name, tag_suggestion};
use crate::time::DateTime;
use crate::{api, config, db, resource, search, update};
use diesel::dsl::{count, max};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list_tags = warp::get()
        .and(api::auth())
        .and(warp::path!("tags"))
        .and(warp::query())
        .map(list_tags)
        .map(api::Reply::from);
    let get_tag = warp::get()
        .and(api::auth())
        .and(warp::path!("tag" / String))
        .and(api::resource_query())
        .map(get_tag)
        .map(api::Reply::from);
    let get_tag_siblings = warp::get()
        .and(api::auth())
        .and(warp::path!("tag-siblings" / String))
        .and(api::resource_query())
        .map(get_tag_siblings)
        .map(api::Reply::from);
    let create_tag = warp::post()
        .and(api::auth())
        .and(warp::path!("tags"))
        .and(api::resource_query())
        .and(warp::body::json())
        .map(create_tag)
        .map(api::Reply::from);
    let merge_tags = warp::post()
        .and(api::auth())
        .and(warp::path!("tag-merge"))
        .and(api::resource_query())
        .and(warp::body::json())
        .map(merge_tags)
        .map(api::Reply::from);
    let update_tag = warp::put()
        .and(api::auth())
        .and(warp::path!("tag" / String))
        .and(api::resource_query())
        .and(warp::body::json())
        .map(update_tag)
        .map(api::Reply::from);
    let delete_tag = warp::delete()
        .and(api::auth())
        .and(warp::path!("tag" / String))
        .and(warp::body::json())
        .map(delete_tag)
        .map(api::Reply::from);

    list_tags
        .or(get_tag)
        .or(get_tag_siblings)
        .or(create_tag)
        .or(merge_tags)
        .or(update_tag)
        .or(delete_tag)
}

const MAX_TAGS_PER_PAGE: i64 = 50;
const MAX_TAG_SIBLINGS: i64 = 50;

fn create_field_table(fields: Option<&str>) -> Result<FieldTable<bool>, Box<dyn std::error::Error>> {
    fields
        .map(resource::tag::Field::create_table)
        .transpose()
        .map(|opt_table| opt_table.unwrap_or(FieldTable::filled(true)))
        .map_err(Box::from)
}

fn list_tags(auth: AuthResult, query: PagedQuery) -> ApiResult<PagedResponse<TagInfo>> {
    let client = auth?;
    query.bump_login(client)?;
    api::verify_privilege(client, config::privileges().tag_list)?;

    let offset = query.offset.unwrap_or(0);
    let limit = std::cmp::min(query.limit.get(), MAX_TAGS_PER_PAGE);
    let fields = create_field_table(query.fields())?;

    db::get_connection()?.transaction(|conn| {
        let mut search_criteria = search::tag::parse_search_criteria(query.criteria())?;
        search_criteria.add_offset_and_limit(offset, limit);
        let sql_query = search::tag::build_query(&search_criteria)?;

        let total = if search_criteria.has_filter() {
            let count_query = search::tag::build_query(&search_criteria)?;
            count_query.count().first(conn)?
        } else {
            let tag_count: i32 = database_statistics::table
                .select(database_statistics::tag_count)
                .first(conn)?;
            i64::from(tag_count)
        };

        let selected_tags: Vec<i32> = search::tag::get_ordered_ids(conn, sql_query, &search_criteria)?;
        Ok(PagedResponse {
            query: query.query.query,
            offset,
            limit,
            total,
            results: TagInfo::new_batch_from_ids(conn, selected_tags, &fields)?,
        })
    })
}

fn get_tag(auth: AuthResult, name: String, query: ResourceQuery) -> ApiResult<TagInfo> {
    let client = auth?;
    query.bump_login(client)?;
    api::verify_privilege(client, config::privileges().tag_view)?;

    let fields = create_field_table(query.fields())?;
    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    db::get_connection()?.transaction(|conn| {
        let tag_id = tag_name::table
            .select(tag_name::tag_id)
            .filter(tag_name::name.eq(name))
            .first(conn)
            .optional()?
            .ok_or(api::Error::NotFound(ResourceType::Tag))?;
        TagInfo::new_from_id(conn, tag_id, &fields).map_err(api::Error::from)
    })
}

#[derive(Serialize)]
struct TagSibling {
    tag: TagInfo,
    occurrences: i64,
}

#[derive(Serialize)]
struct TagSiblings {
    results: Vec<TagSibling>,
}

fn get_tag_siblings(auth: AuthResult, name: String, query: ResourceQuery) -> ApiResult<TagSiblings> {
    let client = auth?;
    query.bump_login(client)?;
    api::verify_privilege(client, config::privileges().tag_view)?;

    let fields = create_field_table(query.fields())?;
    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    db::get_connection()?.transaction(|conn| {
        let tag_id: i32 = tag::table
            .select(tag::id)
            .inner_join(tag_name::table)
            .filter(tag_name::name.eq(name))
            .first(conn)?;
        let posts_tagged_on = post_tag::table
            .select(post_tag::post_id)
            .filter(post_tag::tag_id.eq(tag_id))
            .into_boxed();
        let (sibling_ids, common_post_counts): (_, Vec<_>) = post_tag::table
            .group_by(post_tag::tag_id)
            .select((post_tag::tag_id, count(post_tag::post_id)))
            .filter(post_tag::post_id.eq_any(posts_tagged_on))
            .filter(post_tag::tag_id.ne(tag_id))
            .order_by(count(post_tag::post_id).desc())
            .limit(MAX_TAG_SIBLINGS)
            .load::<(i32, i64)>(conn)?
            .into_iter()
            .unzip();

        let siblings = TagInfo::new_batch_from_ids(conn, sibling_ids, &fields)?
            .into_iter()
            .zip(common_post_counts)
            .map(|(tag, occurrences)| TagSibling { tag, occurrences })
            .collect();
        Ok(TagSiblings { results: siblings })
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewTagInfo {
    category: String,
    description: Option<String>,
    names: Vec<String>,
    implications: Option<Vec<String>>,
    suggestions: Option<Vec<String>>,
}

fn create_tag(auth: AuthResult, query: ResourceQuery, tag_info: NewTagInfo) -> ApiResult<TagInfo> {
    let client = auth?;
    query.bump_login(client)?;
    api::verify_privilege(client, config::privileges().tag_create)?;

    if tag_info.names.is_empty() {
        return Err(api::Error::NoNamesGiven(ResourceType::Tag));
    }

    let fields = create_field_table(query.fields())?;
    let mut conn = db::get_connection()?;
    let tag_id = conn.transaction(|conn| {
        let category_id: i32 = tag_category::table
            .select(tag_category::id)
            .filter(tag_category::name.eq(tag_info.category))
            .first(conn)?;
        let new_tag = NewTag {
            category_id,
            description: tag_info.description.as_deref().unwrap_or(""),
        };
        let tag_id: i32 = diesel::insert_into(tag::table)
            .values(new_tag)
            .returning(tag::id)
            .get_result(conn)?;

        update::tag::add_names(conn, tag_id, 0, tag_info.names)?;
        if let Some(implications) = tag_info.implications {
            let implied_ids = update::tag::get_or_create_tag_ids(conn, client, implications, true)?;
            update::tag::add_implications(conn, tag_id, implied_ids)?;
        }
        if let Some(suggestions) = tag_info.suggestions {
            let suggested_ids = update::tag::get_or_create_tag_ids(conn, client, suggestions, true)?;
            update::tag::add_suggestions(conn, tag_id, suggested_ids)?;
        }
        Ok::<_, api::Error>(tag_id)
    })?;
    conn.transaction(|conn| TagInfo::new_from_id(conn, tag_id, &fields).map_err(api::Error::from))
}

fn merge_tags(auth: AuthResult, query: ResourceQuery, merge_info: MergeRequest<String>) -> ApiResult<TagInfo> {
    let client = auth?;
    query.bump_login(client)?;
    api::verify_privilege(client, config::privileges().tag_merge)?;

    let get_tag_info = |conn: &mut PgConnection, name: String| {
        tag::table
            .select((tag::id, tag::last_edit_time))
            .inner_join(tag_name::table)
            .filter(tag_name::name.eq(name))
            .first(conn)
    };

    let fields = create_field_table(query.fields())?;
    let mut conn = db::get_connection()?;
    let merged_tag_id = conn.transaction(|conn| {
        let (remove_id, remove_version) = get_tag_info(conn, merge_info.remove)?;
        let (merge_to_id, merge_to_version) = get_tag_info(conn, merge_info.merge_to)?;
        if remove_id == merge_to_id {
            return Err(api::Error::SelfMerge(ResourceType::Tag));
        }
        api::verify_version(remove_version, merge_info.remove_version)?;
        api::verify_version(merge_to_version, merge_info.merge_to_version)?;

        // Merge implications
        let involved_implications: Vec<TagImplication> = tag_implication::table
            .filter(tag_implication::parent_id.eq(remove_id))
            .or_filter(tag_implication::child_id.eq(remove_id))
            .or_filter(tag_implication::parent_id.eq(merge_to_id))
            .or_filter(tag_implication::child_id.eq(merge_to_id))
            .load(conn)?;
        let merged_implications: HashSet<_> = involved_implications
            .iter()
            .copied()
            .map(|mut implication| {
                if implication.parent_id == remove_id {
                    implication.parent_id = merge_to_id
                } else if implication.child_id == remove_id {
                    implication.child_id = merge_to_id
                }
                implication
            })
            .filter(|implication| implication.parent_id != implication.child_id)
            .collect();
        diesel::delete(tag_implication::table)
            .filter(tag_implication::parent_id.eq(merge_to_id))
            .or_filter(tag_implication::child_id.eq(merge_to_id))
            .execute(conn)?;
        diesel::insert_into(tag_implication::table)
            .values(merged_implications.into_iter().collect::<Vec<_>>())
            .execute(conn)?;

        // Merge suggestions
        let involved_suggestions: Vec<TagSuggestion> = tag_suggestion::table
            .filter(tag_suggestion::parent_id.eq(remove_id))
            .or_filter(tag_suggestion::child_id.eq(remove_id))
            .or_filter(tag_suggestion::parent_id.eq(merge_to_id))
            .or_filter(tag_suggestion::child_id.eq(merge_to_id))
            .load(conn)?;
        let merged_suggestions: HashSet<_> = involved_suggestions
            .iter()
            .copied()
            .map(|mut suggestion| {
                if suggestion.parent_id == remove_id {
                    suggestion.parent_id = merge_to_id
                } else if suggestion.child_id == remove_id {
                    suggestion.child_id = merge_to_id
                }
                suggestion
            })
            .filter(|suggestion| suggestion.parent_id != suggestion.child_id)
            .collect();
        diesel::delete(tag_suggestion::table)
            .filter(tag_suggestion::parent_id.eq(merge_to_id))
            .or_filter(tag_suggestion::child_id.eq(merge_to_id))
            .execute(conn)?;
        diesel::insert_into(tag_suggestion::table)
            .values(merged_suggestions.into_iter().collect::<Vec<_>>())
            .execute(conn)?;

        // Merge usages
        let merge_to_posts = post_tag::table
            .select(post_tag::post_id)
            .filter(post_tag::tag_id.eq(merge_to_id))
            .into_boxed();
        let new_post_tags: Vec<_> = post_tag::table
            .select(post_tag::post_id)
            .filter(post_tag::tag_id.eq(remove_id))
            .filter(post_tag::post_id.ne_all(merge_to_posts))
            .load(conn)?
            .into_iter()
            .map(|post_id| PostTag {
                post_id,
                tag_id: merge_to_id,
            })
            .collect();
        diesel::insert_into(post_tag::table)
            .values(new_post_tags)
            .execute(conn)?;

        // Merge names
        let current_name_count = tag_name::table
            .select(max(tag_name::order) + 1)
            .filter(tag_name::tag_id.eq(merge_to_id))
            .first::<Option<_>>(conn)?
            .unwrap_or(0);
        let removed_names = diesel::delete(tag_name::table.filter(tag_name::tag_id.eq(remove_id)))
            .returning(tag_name::name)
            .get_results(conn)?;
        update::tag::add_names(conn, merge_to_id, current_name_count, removed_names)?;

        diesel::delete(tag::table.find(remove_id)).execute(conn)?;
        Ok::<_, api::Error>(merge_to_id)
    })?;
    conn.transaction(|conn| TagInfo::new_from_id(conn, merged_tag_id, &fields).map_err(api::Error::from))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TagUpdate {
    version: DateTime,
    category: Option<String>,
    description: Option<String>,
    names: Option<Vec<String>>,
    implications: Option<Vec<String>>,
    suggestions: Option<Vec<String>>,
}

fn update_tag(auth: AuthResult, name: String, query: ResourceQuery, update: TagUpdate) -> ApiResult<TagInfo> {
    let client = auth?;
    query.bump_login(client)?;

    let fields = create_field_table(query.fields())?;
    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    let mut conn = db::get_connection()?;
    let tag_id = conn.transaction(|conn| {
        let (tag_id, tag_version) = tag::table
            .select((tag::id, tag::last_edit_time))
            .inner_join(tag_name::table)
            .filter(tag_name::name.eq(name))
            .first(conn)?;
        api::verify_version(tag_version, update.version)?;

        if let Some(category) = update.category {
            api::verify_privilege(client, config::privileges().tag_edit_category)?;

            let category_id: i32 = tag_category::table
                .select(tag_category::id)
                .filter(tag_category::name.eq(category))
                .first(conn)?;
            diesel::update(tag::table.find(tag_id))
                .set(tag::category_id.eq(category_id))
                .execute(conn)?;
        }
        if let Some(description) = update.description {
            api::verify_privilege(client, config::privileges().tag_edit_description)?;

            diesel::update(tag::table.find(tag_id))
                .set(tag::description.eq(description))
                .execute(conn)?;
        }
        if let Some(names) = update.names {
            api::verify_privilege(client, config::privileges().tag_edit_name)?;
            if names.is_empty() {
                return Err(api::Error::NoNamesGiven(ResourceType::Tag));
            }

            update::tag::delete_names(conn, tag_id)?;
            update::tag::add_names(conn, tag_id, 0, names)?;
        }
        if let Some(implications) = update.implications {
            api::verify_privilege(client, config::privileges().tag_edit_implication)?;

            let implied_ids = update::tag::get_or_create_tag_ids(conn, client, implications, true)?;
            diesel::delete(tag_implication::table)
                .filter(tag_implication::parent_id.eq(tag_id))
                .execute(conn)?;
            update::tag::add_implications(conn, tag_id, implied_ids)?;
        }
        if let Some(suggestions) = update.suggestions {
            api::verify_privilege(client, config::privileges().tag_edit_suggestion)?;

            let suggested_ids = update::tag::get_or_create_tag_ids(conn, client, suggestions, true)?;
            diesel::delete(tag_suggestion::table)
                .filter(tag_suggestion::parent_id.eq(tag_id))
                .execute(conn)?;
            update::tag::add_suggestions(conn, tag_id, suggested_ids)?;
        }
        Ok::<_, api::Error>(tag_id)
    })?;
    conn.transaction(|conn| TagInfo::new_from_id(conn, tag_id, &fields).map_err(api::Error::from))
}

fn delete_tag(auth: AuthResult, name: String, client_version: DeleteRequest) -> ApiResult<()> {
    let client = auth?;
    api::verify_privilege(client, config::privileges().tag_delete)?;

    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    db::get_connection()?.transaction(|conn| {
        let (tag_id, tag_version): (i32, DateTime) = tag::table
            .select((tag::id, tag::last_edit_time))
            .inner_join(tag_name::table)
            .filter(tag_name::name.eq(name))
            .first(conn)?;
        api::verify_version(tag_version, *client_version)?;

        diesel::delete(tag::table.find(tag_id)).execute(conn)?;
        Ok(())
    })
}
