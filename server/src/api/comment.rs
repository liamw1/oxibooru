use crate::api::{ApiResult, AuthResult, DeleteRequest, PagedQuery, PagedResponse, RatingRequest, ResourceQuery};
use crate::model::comment::{NewComment, NewCommentScore};
use crate::model::enums::{ResourceType, Score};
use crate::resource::comment::{CommentInfo, FieldTable};
use crate::schema::{comment, comment_score, database_statistics};
use crate::time::DateTime;
use crate::{api, config, db, resource, search};
use diesel::dsl::exists;
use diesel::prelude::*;
use serde::Deserialize;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let list_comments = warp::get()
        .and(api::auth())
        .and(warp::path!("comments"))
        .and(warp::query())
        .map(list_comments)
        .map(api::Reply::from);
    let get_comment = warp::get()
        .and(api::auth())
        .and(warp::path!("comment" / i32))
        .and(warp::query())
        .map(get_comment)
        .map(api::Reply::from);
    let create_comment = warp::post()
        .and(api::auth())
        .and(warp::path!("comments"))
        .and(warp::query())
        .and(warp::body::json())
        .map(create_comment)
        .map(api::Reply::from);
    let update_comment = warp::put()
        .and(api::auth())
        .and(warp::path!("comment" / i32))
        .and(warp::query())
        .and(warp::body::json())
        .map(update_comment)
        .map(api::Reply::from);
    let rate_comment = warp::put()
        .and(api::auth())
        .and(warp::path!("comment" / i32 / "score"))
        .and(warp::query())
        .and(warp::body::json())
        .map(rate_comment)
        .map(api::Reply::from);
    let delete_comment = warp::delete()
        .and(api::auth())
        .and(warp::path!("comment" / i32))
        .and(warp::body::json())
        .map(delete_comment)
        .map(api::Reply::from);

    list_comments
        .or(get_comment)
        .or(create_comment)
        .or(update_comment)
        .or(rate_comment)
        .or(delete_comment)
}

const MAX_COMMENTS_PER_PAGE: i64 = 50;

fn create_field_table(fields: Option<&str>) -> Result<FieldTable<bool>, Box<dyn std::error::Error>> {
    fields
        .map(resource::comment::Field::create_table)
        .transpose()
        .map(|opt_table| opt_table.unwrap_or(FieldTable::filled(true)))
        .map_err(Box::from)
}

fn list_comments(auth: AuthResult, query: PagedQuery) -> ApiResult<PagedResponse<CommentInfo>> {
    let client = auth?;
    api::verify_privilege(client, config::privileges().comment_list)?;

    let client_id = client.map(|user| user.id);
    let offset = query.offset.unwrap_or(0);
    let limit = std::cmp::min(query.limit.get(), MAX_COMMENTS_PER_PAGE);
    let fields = create_field_table(query.fields())?;

    db::get_connection()?.transaction(|conn| {
        let mut search_criteria = search::comment::parse_search_criteria(query.criteria())?;
        search_criteria.add_offset_and_limit(offset, limit);
        let sql_query = search::comment::build_query(&search_criteria)?;

        let total = if search_criteria.has_filter() {
            let count_query = search::comment::build_query(&search_criteria)?;
            count_query.count().first(conn)?
        } else {
            let comment_count: i32 = database_statistics::table
                .select(database_statistics::comment_count)
                .first(conn)?;
            i64::from(comment_count)
        };

        let selected_comments: Vec<i32> = search::comment::get_ordered_ids(conn, sql_query, &search_criteria)?;
        Ok(PagedResponse {
            query: query.query.query,
            offset,
            limit,
            total,
            results: CommentInfo::new_batch_from_ids(conn, client_id, selected_comments, &fields)?,
        })
    })
}

fn get_comment(auth: AuthResult, comment_id: i32, query: ResourceQuery) -> ApiResult<CommentInfo> {
    let client = auth?;
    api::verify_privilege(client, config::privileges().comment_view)?;

    let client_id = client.map(|user| user.id);
    let fields = create_field_table(query.fields())?;
    db::get_connection()?.transaction(|conn| {
        let comment_exists: bool = diesel::select(exists(comment::table.find(comment_id))).get_result(conn)?;
        if !comment_exists {
            return Err(api::Error::NotFound(ResourceType::Comment));
        }
        CommentInfo::new_from_id(conn, client_id, comment_id, &fields).map_err(api::Error::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct NewCommentInfo {
    post_id: i32,
    text: String,
}

fn create_comment(auth: AuthResult, query: ResourceQuery, comment_info: NewCommentInfo) -> ApiResult<CommentInfo> {
    let client = auth?;
    api::verify_privilege(client, config::privileges().comment_create)?;

    let user_id = client.ok_or(api::Error::NotLoggedIn).map(|user| user.id)?;
    let fields = create_field_table(query.fields())?;
    let new_comment = NewComment {
        user_id: Some(user_id),
        post_id: comment_info.post_id,
        text: &comment_info.text,
        creation_time: DateTime::now(),
    };

    let mut conn = db::get_connection()?;
    let comment_id: i32 = diesel::insert_into(comment::table)
        .values(new_comment)
        .returning(comment::id)
        .get_result(&mut conn)?;
    conn.transaction(|conn| {
        CommentInfo::new_from_id(conn, Some(user_id), comment_id, &fields).map_err(api::Error::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommentUpdate {
    version: DateTime,
    text: String,
}

fn update_comment(
    auth: AuthResult,
    comment_id: i32,
    query: ResourceQuery,
    update: CommentUpdate,
) -> ApiResult<CommentInfo> {
    let client = auth?;
    let client_id = client.map(|user| user.id);
    let fields = create_field_table(query.fields())?;

    let mut conn = db::get_connection()?;
    conn.transaction(|conn| {
        let (comment_owner, comment_version): (Option<i32>, DateTime) = comment::table
            .find(comment_id)
            .select((comment::user_id, comment::last_edit_time))
            .first(conn)?;
        api::verify_version(comment_version, update.version)?;

        let required_rank = match client_id.is_some() && client_id == comment_owner {
            true => config::privileges().comment_edit_own,
            false => config::privileges().comment_edit_any,
        };
        api::verify_privilege(client, required_rank)?;

        diesel::update(comment::table.find(comment_id))
            .set(comment::text.eq(update.text))
            .execute(conn)
            .map_err(api::Error::from)
    })?;
    conn.transaction(|conn| CommentInfo::new_from_id(conn, client_id, comment_id, &fields).map_err(api::Error::from))
}

fn rate_comment(
    auth: AuthResult,
    comment_id: i32,
    query: ResourceQuery,
    rating: RatingRequest,
) -> ApiResult<CommentInfo> {
    let client = auth?;
    api::verify_privilege(client, config::privileges().comment_score)?;

    let user_id = client.ok_or(api::Error::NotLoggedIn).map(|user| user.id)?;
    let fields = create_field_table(query.fields())?;

    let mut conn = db::get_connection()?;
    conn.transaction(|conn| {
        diesel::delete(comment_score::table.find((comment_id, user_id))).execute(conn)?;

        if let Ok(score) = Score::try_from(*rating) {
            let new_comment_score = NewCommentScore {
                comment_id,
                user_id,
                score,
            };
            diesel::insert_into(comment_score::table)
                .values(new_comment_score)
                .execute(conn)?;
        }
        Ok::<_, api::Error>(())
    })?;
    conn.transaction(|conn| {
        CommentInfo::new_from_id(conn, Some(user_id), comment_id, &fields).map_err(api::Error::from)
    })
}

fn delete_comment(auth: AuthResult, comment_id: i32, client_version: DeleteRequest) -> ApiResult<()> {
    let client = auth?;
    let client_id = client.map(|user| user.id);

    db::get_connection()?.transaction(|conn| {
        let (comment_owner, comment_version): (Option<i32>, DateTime) = comment::table
            .find(comment_id)
            .select((comment::user_id, comment::last_edit_time))
            .first(conn)?;
        api::verify_version(comment_version, *client_version)?;

        let required_rank = match client_id.is_some() && client_id == comment_owner {
            true => config::privileges().comment_delete_own,
            false => config::privileges().comment_delete_any,
        };
        api::verify_privilege(client, required_rank)?;

        diesel::delete(comment::table.find(comment_id)).execute(conn)?;
        Ok(())
    })
}

#[cfg(test)]
mod test {
    use crate::test::*;

    const FIELDS: &str = "fields=id%2CpostId%2Ctext%2Cuser%2Cscore%2CownScore";

    #[tokio::test]
    async fn list() {
        verify_query(&format!("GET /comments/?query=-sort:id&limit=42&{FIELDS}"), "comment/list.json").await;
    }
}
