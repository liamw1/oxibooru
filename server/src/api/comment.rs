use crate::api::{ApiResult, AuthResult, PagedQuery, PagedResponse};
use crate::resource::comment::CommentInfo;
use crate::{api, config, search};
use diesel::prelude::*;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list_comments = warp::get()
        .and(warp::path!("comments"))
        .and(api::auth())
        .and(warp::query())
        .map(list_comments)
        .map(api::Reply::from);

    list_comments
}

type PagedCommentInfo = PagedResponse<CommentInfo>;

const MAX_COMMENTS_PER_PAGE: i64 = 50;

fn list_comments(auth: AuthResult, query: PagedQuery) -> ApiResult<PagedCommentInfo> {
    let _timer = crate::util::Timer::new("list_comments");

    let client = auth?;
    api::verify_privilege(client.as_ref(), config::privileges().comment_list)?;

    let client_id = client.map(|user| user.id);
    let offset = query.offset.unwrap_or(0);
    let limit = std::cmp::min(query.limit, MAX_COMMENTS_PER_PAGE);

    crate::establish_connection()?.transaction(|conn| {
        let mut search_criteria = search::comment::parse_search_criteria(query.criteria())?;
        search_criteria.add_offset_and_limit(offset, limit);
        let count_query = search::comment::build_query(&search_criteria)?;
        let sql_query = search::comment::build_query(&search_criteria)?;

        let total = count_query.count().first(conn)?;
        let selected_tags: Vec<i32> = search::comment::get_ordered_ids(conn, sql_query, &search_criteria)?;
        Ok(PagedCommentInfo {
            query: query.query.query,
            offset,
            limit,
            total,
            results: CommentInfo::new_batch_from_ids(conn, client_id, selected_tags)?,
        })
    })
}
