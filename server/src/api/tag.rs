use crate::api::{ApiResult, AuthResult, PagedQuery, PagedResponse};
use crate::resource::tag::{FieldTable, TagInfo};
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

    list_tags
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

    let mut search_criteria = search::tag::parse_search_criteria(query.criteria())?;
    search_criteria.add_offset_and_limit(offset, limit);
    let count_query = search::tag::build_query(&search_criteria)?;
    let sql_query = search::tag::build_query(&search_criteria)?;

    println!("SQL Query: {}\n", diesel::debug_query(&sql_query).to_string());

    let mut conn = crate::establish_connection()?;
    let total = count_query.count().first(&mut conn)?;
    let selected_tags: Vec<i32> = search::tag::get_ordered_ids(&mut conn, sql_query, &search_criteria)?;

    Ok(PagedTagInfo {
        query: query.query.query,
        offset,
        limit,
        total,
        results: TagInfo::new_batch_from_ids(&mut conn, selected_tags, &fields)?,
    })
}
