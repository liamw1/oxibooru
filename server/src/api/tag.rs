use crate::api::{ApiResult, AuthResult, PagedQuery, PagedResponse, ResourceQuery};
use crate::resource::tag::{FieldTable, TagInfo};
use crate::schema::tag_name;
use crate::{api, config, resource, search};
use diesel::prelude::*;
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

    list_tags.or(get_tag)
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
    crate::establish_connection()?.transaction(|conn| {
        let tag_id = tag_name::table
            .select(tag_name::tag_id)
            .filter(tag_name::name.eq(name))
            .first(conn)?;
        TagInfo::new_from_id(conn, tag_id, &fields).map_err(api::Error::from)
    })
}
