use crate::api::{ApiResult, AuthResult, PagedQuery, PagedResponse};
use crate::resource::pool::{FieldTable, PoolInfo};
use crate::{api, config, resource, search};
use diesel::prelude::*;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list_pools = warp::get()
        .and(warp::path!("pools"))
        .and(api::auth())
        .and(warp::query())
        .map(list_pools)
        .map(api::Reply::from);

    list_pools
}

type PagedPoolInfo = PagedResponse<PoolInfo>;

const MAX_POOLS_PER_PAGE: i64 = 50;

fn create_field_table(fields: Option<&str>) -> Result<FieldTable<bool>, Box<dyn std::error::Error>> {
    fields
        .map(resource::pool::Field::create_table)
        .transpose()
        .map(|opt_table| opt_table.unwrap_or(FieldTable::filled(true)))
        .map_err(Box::from)
}

fn list_pools(auth: AuthResult, query: PagedQuery) -> ApiResult<PagedPoolInfo> {
    let _timer = crate::util::Timer::new("list_pools");

    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().pool_list)?;

    let offset = query.offset.unwrap_or(0);
    let limit = std::cmp::min(query.limit, MAX_POOLS_PER_PAGE);
    let fields = create_field_table(query.fields())?;

    let mut search_criteria = search::pool::parse_search_criteria(query.criteria())?;
    search_criteria.add_offset_and_limit(offset, limit);
    let count_query = search::pool::build_query(&search_criteria)?;
    let sql_query = search::pool::build_query(&search_criteria)?;

    println!("SQL Query: {}\n", diesel::debug_query(&sql_query).to_string());

    let mut conn = crate::establish_connection()?;
    let total = count_query.count().first(&mut conn)?;
    let selected_tags: Vec<i32> = search::pool::get_ordered_ids(&mut conn, sql_query, &search_criteria)?;

    Ok(PagedPoolInfo {
        query: query.query.query,
        offset,
        limit,
        total,
        results: PoolInfo::new_batch_from_ids(&mut conn, selected_tags, &fields)?,
    })
}
