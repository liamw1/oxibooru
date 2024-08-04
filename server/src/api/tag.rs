use crate::api::{ApiResult, AuthResult, DeleteRequest, MergeRequest, PagedQuery, PagedResponse, ResourceQuery};
use crate::model::tag::NewTag;
use crate::resource::tag::{FieldTable, TagInfo};
use crate::schema::{post_tag, tag, tag_category, tag_implication, tag_name, tag_suggestion};
use crate::util::DateTime;
use crate::{api, config, resource, search, update};
use diesel::dsl::*;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
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
    let get_tag_siblings = warp::get()
        .and(warp::path!("tag-siblings" / String))
        .and(api::auth())
        .and(api::resource_query())
        .map(get_tag_siblings)
        .map(api::Reply::from);
    let create_tag = warp::post()
        .and(warp::path!("tags"))
        .and(api::auth())
        .and(api::resource_query())
        .and(warp::body::json())
        .map(create_tag)
        .map(api::Reply::from);
    let merge_tags = warp::post()
        .and(warp::path!("tag-merge"))
        .and(api::auth())
        .and(api::resource_query())
        .and(warp::body::json())
        .map(merge_tags)
        .map(api::Reply::from);
    let update_tag = warp::put()
        .and(warp::path!("tag" / String))
        .and(api::auth())
        .and(api::resource_query())
        .and(warp::body::json())
        .map(update_tag)
        .map(api::Reply::from);
    let delete_tag = warp::delete()
        .and(warp::path!("tag" / String))
        .and(api::auth())
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
    let _timer = crate::util::Timer::new("list_tags");

    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().tag_list)?;

    let offset = query.offset.unwrap_or(0);
    let limit = std::cmp::min(query.limit.get(), MAX_TAGS_PER_PAGE);
    let fields = create_field_table(query.fields())?;

    crate::establish_connection()?.transaction(|conn| {
        let mut search_criteria = search::tag::parse_search_criteria(query.criteria())?;
        search_criteria.add_offset_and_limit(offset, limit);
        let count_query = search::tag::build_query(&search_criteria)?;
        let sql_query = search::tag::build_query(&search_criteria)?;

        let total = count_query.count().first(conn)?;
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

#[derive(Serialize)]
struct TagSibling {
    tag: TagInfo,
    occurrences: i64,
}

#[derive(Serialize)]
struct TagSiblings {
    results: Vec<TagSibling>,
}

fn get_tag_siblings(name: String, auth: AuthResult, query: ResourceQuery) -> ApiResult<TagSiblings> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().tag_view)?;

    let fields = create_field_table(query.fields())?;
    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    crate::establish_connection()?.transaction(|conn| {
        let tag_id: i32 = tag::table
            .select(tag::id)
            .inner_join(tag_name::table)
            .filter(tag_name::name.eq(name))
            .first(conn)?;
        let posts_tagged_on: Vec<i32> = post_tag::table
            .select(post_tag::post_id)
            .filter(post_tag::tag_id.eq(tag_id))
            .load(conn)?;
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
            .zip(common_post_counts.into_iter())
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
    api::verify_privilege(client.as_ref(), config::privileges().tag_create)?;

    if tag_info.names.is_empty() {
        return Err(api::Error::NoNamesGiven);
    }

    let fields = create_field_table(query.fields())?;
    crate::establish_connection()?.transaction(|conn| {
        let category_id: i32 = tag_category::table
            .select(tag_category::id)
            .filter(tag_category::name.eq(tag_info.category))
            .first(conn)?;
        let new_tag = NewTag { category_id };
        let tag_id: i32 = diesel::insert_into(tag::table)
            .values(new_tag)
            .returning(tag::id)
            .get_result(conn)?;

        if let Some(description) = tag_info.description {
            update::tag::description(conn, tag_id, description)?;
        }

        update::tag::add_names(conn, tag_id, 0, tag_info.names)?;
        if let Some(implications) = tag_info.implications {
            let implied_ids = update::tag::get_or_create_tag_ids(conn, client.as_ref(), implications)?;
            update::tag::add_implications(conn, tag_id, implied_ids)?;
        }
        if let Some(suggestions) = tag_info.suggestions {
            let suggested_ids = update::tag::get_or_create_tag_ids(conn, client.as_ref(), suggestions)?;
            update::tag::add_suggestions(conn, tag_id, suggested_ids)?;
        }

        TagInfo::new_from_id(conn, tag_id, &fields).map_err(api::Error::from)
    })
}

fn merge_tags(auth: AuthResult, query: ResourceQuery, merge_info: MergeRequest<String>) -> ApiResult<TagInfo> {
    let _timer = crate::util::Timer::new("merge_tags");

    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().tag_merge)?;

    let get_tag_info = |conn: &mut PgConnection, name: String| {
        tag::table
            .select((tag::id, tag::last_edit_time))
            .inner_join(tag_name::table)
            .filter(tag_name::name.eq(name))
            .first(conn)
    };

    let fields = create_field_table(query.fields())?;
    crate::establish_connection()?.transaction(|conn| {
        let (remove_id, remove_version) = get_tag_info(conn, merge_info.remove)?;
        let (merge_to_id, merge_to_version) = get_tag_info(conn, merge_info.merge_to)?;
        if remove_id == merge_to_id {
            return Err(api::Error::SelfMerge);
        }
        api::verify_version(remove_version, merge_info.remove_version)?;
        api::verify_version(merge_to_version, merge_info.merge_to_version)?;

        // Merge implications
        let merged_implications: Vec<i32> = tag_implication::table
            .select(tag_implication::child_id)
            .filter(tag_implication::parent_id.eq(remove_id))
            .or_filter(tag_implication::parent_id.eq(merge_to_id))
            .load(conn)?;
        update::tag::delete_implications(conn, merge_to_id)?;
        update::tag::add_implications(conn, merge_to_id, merged_implications)?;

        // Merge suggestions
        let merged_suggestions: Vec<i32> = tag_suggestion::table
            .select(tag_suggestion::child_id)
            .filter(tag_suggestion::parent_id.eq(remove_id))
            .or_filter(tag_suggestion::parent_id.eq(merge_to_id))
            .load(conn)?;
        update::tag::delete_suggestions(conn, merge_to_id)?;
        update::tag::add_suggestions(conn, merge_to_id, merged_suggestions)?;

        // Merge usages
        let merge_to_posts = post_tag::table
            .select(post_tag::post_id)
            .filter(post_tag::tag_id.eq(merge_to_id))
            .into_boxed();
        diesel::update(post_tag::table)
            .filter(post_tag::tag_id.eq(remove_id))
            .filter(post_tag::post_id.ne_all(merge_to_posts))
            .set(post_tag::tag_id.eq(merge_to_id))
            .execute(conn)?;

        diesel::delete(tag::table.find(remove_id)).execute(conn)?;
        TagInfo::new_from_id(conn, merge_to_id, &fields).map_err(api::Error::from)
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
        let (tag_id, tag_version) = tag::table
            .select((tag::id, tag::last_edit_time))
            .inner_join(tag_name::table)
            .filter(tag_name::name.eq(name))
            .first(conn)?;
        api::verify_version(tag_version, update.version)?;

        if let Some(category) = update.category {
            api::verify_privilege(client.as_ref(), config::privileges().tag_edit_category)?;

            let category_id: i32 = tag_category::table
                .select(tag_category::id)
                .filter(tag_category::name.eq(category))
                .first(conn)?;
            diesel::update(tag::table.find(tag_id))
                .set(tag::category_id.eq(category_id))
                .execute(conn)?;
        }
        if let Some(description) = update.description {
            api::verify_privilege(client.as_ref(), config::privileges().tag_edit_description)?;
            update::tag::description(conn, tag_id, description)?;
        }
        if let Some(names) = update.names {
            api::verify_privilege(client.as_ref(), config::privileges().tag_edit_name)?;
            update::tag::delete_names(conn, tag_id)?;
            update::tag::add_names(conn, tag_id, 0, names)?;
        }
        if let Some(implications) = update.implications {
            api::verify_privilege(client.as_ref(), config::privileges().tag_edit_implication)?;

            let implied_ids = update::tag::get_or_create_tag_ids(conn, client.as_ref(), implications)?;
            update::tag::delete_implications(conn, tag_id)?;
            update::tag::add_implications(conn, tag_id, implied_ids)?;
        }
        if let Some(suggestions) = update.suggestions {
            api::verify_privilege(client.as_ref(), config::privileges().tag_edit_suggestion)?;

            let suggested_ids = update::tag::get_or_create_tag_ids(conn, client.as_ref(), suggestions)?;
            update::tag::delete_suggestions(conn, tag_id)?;
            update::tag::add_suggestions(conn, tag_id, suggested_ids)?;
        }

        TagInfo::new_from_id(conn, tag_id, &fields).map_err(api::Error::from)
    })
}

fn delete_tag(name: String, auth: AuthResult, client_version: DeleteRequest) -> ApiResult<()> {
    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().tag_delete)?;

    let name = percent_encoding::percent_decode_str(&name).decode_utf8()?;
    crate::establish_connection()?.transaction(|conn| {
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
