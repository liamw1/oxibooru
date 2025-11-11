use crate::api::{ApiResult, DeleteBody, PageParams, PagedResponse, RatingBody, ResourceParams};
use crate::auth::Client;
use crate::model::comment::{NewComment, NewCommentScore};
use crate::model::enums::{ResourceType, Score};
use crate::resource::comment::CommentInfo;
use crate::schema::{comment, comment_score};
use crate::search::Builder;
use crate::search::comment::QueryBuilder;
use crate::time::DateTime;
use crate::{api, config, db, resource};
use axum::extract::{Extension, Path, Query};
use axum::{Json, Router, routing};
use diesel::dsl::exists;
use diesel::prelude::*;
use serde::Deserialize;

pub fn routes() -> Router {
    Router::new()
        .route("/comments", routing::get(list).post(create))
        .route("/comment/{id}", routing::get(get).put(update).delete(delete))
        .route("/comment/{id}/score", routing::put(rate))
}

const MAX_COMMENTS_PER_PAGE: i64 = 1000;

async fn list(
    Extension(client): Extension<Client>,
    Query(params): Query<PageParams>,
) -> ApiResult<Json<PagedResponse<CommentInfo>>> {
    api::verify_privilege(client, config::privileges().comment_list)?;

    let offset = params.offset.unwrap_or(0);
    let limit = std::cmp::min(params.limit.get(), MAX_COMMENTS_PER_PAGE);
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    db::get_connection()?.transaction(|conn| {
        let mut query_builder = QueryBuilder::new(client, params.criteria())?;
        query_builder.set_offset_and_limit(offset, limit);

        let (total, selected_comments) = query_builder.list(conn)?;
        Ok(Json(PagedResponse {
            query: params.into_query(),
            offset,
            limit,
            total,
            results: CommentInfo::new_batch_from_ids(conn, client, &selected_comments, &fields)?,
        }))
    })
}

async fn get(
    Extension(client): Extension<Client>,
    Path(comment_id): Path<i64>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<CommentInfo>> {
    api::verify_privilege(client, config::privileges().comment_view)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    db::get_connection()?.transaction(|conn| {
        let comment_exists: bool = diesel::select(exists(comment::table.find(comment_id))).get_result(conn)?;
        if !comment_exists {
            return Err(api::Error::NotFound(ResourceType::Comment));
        }
        CommentInfo::new_from_id(conn, client, comment_id, &fields)
            .map(Json)
            .map_err(api::Error::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct CreateBody {
    post_id: i64,
    text: String,
}

async fn create(
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<CreateBody>,
) -> ApiResult<Json<CommentInfo>> {
    api::verify_privilege(client, config::privileges().comment_create)?;

    let user_id = client.id.ok_or(api::Error::NotLoggedIn)?;
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let new_comment = NewComment {
        user_id: Some(user_id),
        post_id: body.post_id,
        text: &body.text,
        creation_time: DateTime::now(),
    };

    let mut conn = db::get_connection()?;
    let comment = new_comment.insert_into(comment::table).get_result(&mut conn)?;
    conn.transaction(|conn| CommentInfo::new(conn, client, comment, &fields))
        .map(Json)
        .map_err(api::Error::from)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateBody {
    version: DateTime,
    text: String,
}

async fn update(
    Extension(client): Extension<Client>,
    Path(comment_id): Path<i64>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<UpdateBody>,
) -> ApiResult<Json<CommentInfo>> {
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    let mut conn = db::get_connection()?;
    conn.transaction(|conn| {
        let (comment_owner, comment_version): (Option<i64>, DateTime) = comment::table
            .find(comment_id)
            .select((comment::user_id, comment::last_edit_time))
            .first(conn)?;
        api::verify_version(comment_version, body.version)?;

        let required_rank = if client.id == comment_owner && comment_owner.is_some() {
            config::privileges().comment_edit_own
        } else {
            config::privileges().comment_edit_any
        };
        api::verify_privilege(client, required_rank)?;

        diesel::update(comment::table.find(comment_id))
            .set((comment::text.eq(body.text), comment::last_edit_time.eq(DateTime::now())))
            .execute(conn)
            .map_err(api::Error::from)
    })?;
    conn.transaction(|conn| CommentInfo::new_from_id(conn, client, comment_id, &fields))
        .map(Json)
        .map_err(api::Error::from)
}

async fn rate(
    Extension(client): Extension<Client>,
    Path(comment_id): Path<i64>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<RatingBody>,
) -> ApiResult<Json<CommentInfo>> {
    api::verify_privilege(client, config::privileges().comment_score)?;

    let user_id = client.id.ok_or(api::Error::NotLoggedIn)?;
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    let mut conn = db::get_connection()?;
    conn.transaction(|conn| {
        diesel::delete(comment_score::table.find((comment_id, user_id))).execute(conn)?;

        if let Ok(score) = Score::try_from(*body) {
            NewCommentScore {
                comment_id,
                user_id,
                score,
            }
            .insert_into(comment_score::table)
            .execute(conn)?;
        }
        Ok::<_, api::Error>(())
    })?;
    conn.transaction(|conn| CommentInfo::new_from_id(conn, client, comment_id, &fields))
        .map(Json)
        .map_err(api::Error::from)
}

async fn delete(
    Extension(client): Extension<Client>,
    Path(comment_id): Path<i64>,
    Json(client_version): Json<DeleteBody>,
) -> ApiResult<Json<()>> {
    db::get_connection()?.transaction(|conn| {
        let (comment_owner, comment_version): (Option<i64>, DateTime) = comment::table
            .find(comment_id)
            .select((comment::user_id, comment::last_edit_time))
            .first(conn)?;
        api::verify_version(comment_version, *client_version)?;

        let required_rank = if client.id == comment_owner && comment_owner.is_some() {
            config::privileges().comment_delete_own
        } else {
            config::privileges().comment_delete_any
        };
        api::verify_privilege(client, required_rank)?;

        diesel::delete(comment::table.find(comment_id)).execute(conn)?;
        Ok(Json(()))
    })
}

#[cfg(test)]
mod test {
    use crate::api::ApiResult;
    use crate::model::comment::Comment;
    use crate::schema::{comment, comment_statistics, database_statistics, user, user_statistics};
    use crate::test::*;
    use crate::time::DateTime;
    use diesel::dsl::exists;
    use diesel::prelude::*;
    use serial_test::{parallel, serial};

    // Exclude fields that involve creation_time or last_edit_time
    const FIELDS: &str = "&fields=id,postId,text,user,score,ownScore";

    #[tokio::test]
    #[parallel]
    async fn list() -> ApiResult<()> {
        const QUERY: &str = "GET /comments/?query";
        const SORT: &str = "-sort:id&limit=40";
        verify_query(&format!("{QUERY}={SORT}{FIELDS}"), "comment/list.json").await?;
        verify_query(&format!("{QUERY}=sort:score&limit=1{FIELDS}"), "comment/list_highest_score.json").await?;
        verify_query(&format!("{QUERY}=user:regular_user {SORT}{FIELDS}"), "comment/list_regular_user.json").await?;
        verify_query(&format!("{QUERY}=text:*this* {SORT}{FIELDS}"), "comment/list_text_filter.json").await
    }

    #[tokio::test]
    #[parallel]
    async fn get() -> ApiResult<()> {
        const COMMENT_ID: i64 = 3;
        let get_last_edit_time = |conn: &mut PgConnection| -> QueryResult<DateTime> {
            comment::table
                .select(comment::last_edit_time)
                .filter(comment::id.eq(COMMENT_ID))
                .first(conn)
        };

        let mut conn = get_connection()?;
        let last_edit_time = get_last_edit_time(&mut conn)?;

        verify_query(&format!("GET /comment/{COMMENT_ID}/?{FIELDS}"), "comment/get.json").await?;

        let new_last_edit_time = get_last_edit_time(&mut conn)?;
        assert_eq!(new_last_edit_time, last_edit_time);
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn create() -> ApiResult<()> {
        let get_comment_counts = |conn: &mut PgConnection| -> QueryResult<(i64, i64)> {
            let comment_count = database_statistics::table
                .select(database_statistics::comment_count)
                .first(conn)?;
            let admin_comment_count = user::table
                .inner_join(user_statistics::table)
                .select(user_statistics::comment_count)
                .filter(user::name.eq("administrator"))
                .first(conn)?;
            Ok((comment_count, admin_comment_count))
        };

        let mut conn = get_connection()?;
        let (comment_count, admin_comment_count) = get_comment_counts(&mut conn)?;

        verify_query(&format!("POST /comments/?{FIELDS}"), "comment/create.json").await?;

        let comment_id: i64 = comment::table
            .select(comment::id)
            .order_by(comment::id.desc())
            .first(&mut conn)?;

        let (new_comment_count, new_admin_comment_count) = get_comment_counts(&mut conn)?;
        let comment_score: i64 = comment_statistics::table
            .select(comment_statistics::score)
            .filter(comment_statistics::comment_id.eq(comment_id))
            .first(&mut conn)?;
        assert_eq!(new_comment_count, comment_count + 1);
        assert_eq!(new_admin_comment_count, admin_comment_count + 1);
        assert_eq!(comment_score, 0);

        verify_query(&format!("DELETE /comment/{comment_id}"), "comment/delete.json").await?;

        let (new_comment_count, new_admin_comment_count) = get_comment_counts(&mut conn)?;
        let has_comment: bool = diesel::select(exists(comment::table.find(comment_id))).get_result(&mut conn)?;
        assert_eq!(new_comment_count, comment_count);
        assert_eq!(new_admin_comment_count, admin_comment_count);
        assert!(!has_comment);
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn update() -> ApiResult<()> {
        const COMMENT_ID: i64 = 4;
        let get_comment_info = |conn: &mut PgConnection| -> QueryResult<(Comment, i64)> {
            comment::table
                .inner_join(comment_statistics::table)
                .select((Comment::as_select(), comment_statistics::score))
                .filter(comment::id.eq(COMMENT_ID))
                .first(conn)
        };

        let mut conn = get_connection()?;
        let (comment, score) = get_comment_info(&mut conn)?;

        verify_query(&format!("PUT /comment/{COMMENT_ID}/?{FIELDS}"), "comment/update.json").await?;

        let (new_comment, new_score) = get_comment_info(&mut conn)?;
        assert_ne!(new_comment.text, comment.text);
        assert_eq!(new_comment.creation_time, comment.creation_time);
        assert!(new_comment.last_edit_time > comment.last_edit_time);
        assert_eq!(new_score, score);

        verify_query(&format!("PUT /comment/{COMMENT_ID}/?{FIELDS}"), "comment/update_restore.json").await?;

        let (new_comment, new_score) = get_comment_info(&mut conn)?;
        assert_eq!(new_comment.text, comment.text);
        assert_eq!(new_comment.creation_time, comment.creation_time);
        assert!(new_comment.last_edit_time > comment.last_edit_time);
        assert_eq!(new_score, score);
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn rate() -> ApiResult<()> {
        const COMMENT_ID: i64 = 2;
        let get_comment_info = |conn: &mut PgConnection| -> QueryResult<(i64, DateTime)> {
            comment::table
                .inner_join(comment_statistics::table)
                .select((comment_statistics::score, comment::last_edit_time))
                .filter(comment::id.eq(COMMENT_ID))
                .first(conn)
        };

        let mut conn = get_connection()?;
        let (score, last_edit_time) = get_comment_info(&mut conn)?;

        verify_query(&format!("PUT /comment/{COMMENT_ID}/score/?{FIELDS}"), "comment/like.json").await?;

        let (new_score, new_last_edit_time) = get_comment_info(&mut conn)?;
        assert_eq!(new_score, score + 1);
        assert_eq!(new_last_edit_time, last_edit_time);

        verify_query(&format!("PUT /comment/{COMMENT_ID}/score/?{FIELDS}"), "comment/dislike.json").await?;

        let (new_score, new_last_edit_time) = get_comment_info(&mut conn)?;
        assert_eq!(new_score, score - 1);
        assert_eq!(new_last_edit_time, last_edit_time);

        verify_query(&format!("PUT /comment/{COMMENT_ID}/score/?{FIELDS}"), "comment/remove_score.json").await?;

        let (new_score, new_last_edit_time) = get_comment_info(&mut conn)?;
        assert_eq!(new_score, score);
        assert_eq!(new_last_edit_time, last_edit_time);
        Ok(())
    }
}
