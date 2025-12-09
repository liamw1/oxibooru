use crate::api::error::{ApiError, ApiResult};
use crate::api::extract::{Json, Path, Query};
use crate::api::{DeleteBody, PageParams, PagedResponse, RatingBody, ResourceParams, error};
use crate::app::AppState;
use crate::auth::Client;
use crate::model::comment::{NewComment, NewCommentScore};
use crate::model::enums::{ResourceType, Score};
use crate::resource::comment::CommentInfo;
use crate::schema::{comment, comment_score};
use crate::search::Builder;
use crate::search::comment::QueryBuilder;
use crate::time::DateTime;
use crate::{api, resource};
use axum::extract::{Extension, State};
use axum::{Router, routing};
use diesel::dsl::exists;
use diesel::{Connection, ExpressionMethods, Insertable, OptionalExtension, QueryDsl, RunQueryDsl};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/comments", routing::get(list).post(create))
        .route("/comment/{id}", routing::get(get).put(update).delete(delete))
        .route("/comment/{id}/score", routing::put(rate))
}

const MAX_COMMENTS_PER_PAGE: i64 = 1000;

/// See [listing-comments](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#listing-comments)
async fn list(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Query(params): Query<PageParams>,
) -> ApiResult<Json<PagedResponse<CommentInfo>>> {
    api::verify_privilege(client, state.config.privileges().comment_list)?;

    let offset = params.offset.unwrap_or(0);
    let limit = std::cmp::min(params.limit.get(), MAX_COMMENTS_PER_PAGE);
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    state.get_connection()?.transaction(|conn| {
        let mut query_builder = QueryBuilder::new(client, params.criteria())?;
        query_builder.set_offset_and_limit(offset, limit);

        let (total, selected_comments) = query_builder.list(conn)?;
        Ok(Json(PagedResponse {
            query: params.into_query(),
            offset,
            limit,
            total,
            results: CommentInfo::new_batch_from_ids(conn, &state.config, client, &selected_comments, &fields)?,
        }))
    })
}

/// See [getting-comment](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#getting-comment)
async fn get(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(comment_id): Path<i64>,
    Query(params): Query<ResourceParams>,
) -> ApiResult<Json<CommentInfo>> {
    api::verify_privilege(client, state.config.privileges().comment_view)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    state.get_connection()?.transaction(|conn| {
        let comment_exists: bool = diesel::select(exists(comment::table.find(comment_id))).first(conn)?;
        if !comment_exists {
            return Err(ApiError::NotFound(ResourceType::Comment));
        }
        CommentInfo::new_from_id(conn, &state.config, client, comment_id, &fields)
            .map(Json)
            .map_err(ApiError::from)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct CreateBody {
    post_id: i64,
    text: String,
}

/// See [creating-comment](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#creating-comment)
async fn create(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<CreateBody>,
) -> ApiResult<Json<CommentInfo>> {
    api::verify_privilege(client, state.config.privileges().comment_create)?;

    let fields = resource::create_table(params.fields()).map_err(Box::from)?;
    let new_comment = NewComment {
        user_id: client.id,
        post_id: body.post_id,
        text: &body.text,
        creation_time: DateTime::now(),
    };

    let mut conn = state.get_connection()?;
    let comment = state.get_connection()?.transaction(|conn| {
        let insert_result = new_comment.insert_into(comment::table).get_result(conn);
        error::map_foreign_key_violation(insert_result, ResourceType::Post)
    })?;
    conn.transaction(|conn| CommentInfo::new(conn, &state.config, client, comment, &fields))
        .map(Json)
        .map_err(ApiError::from)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateBody {
    version: DateTime,
    text: String,
}

/// See [updating-comment](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#updating-comment)
async fn update(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(comment_id): Path<i64>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<UpdateBody>,
) -> ApiResult<Json<CommentInfo>> {
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    let mut conn = state.get_connection()?;
    conn.transaction(|conn| {
        let (comment_owner, comment_version): (Option<i64>, DateTime) = comment::table
            .find(comment_id)
            .select((comment::user_id, comment::last_edit_time))
            .first(conn)
            .optional()?
            .ok_or(ApiError::NotFound(ResourceType::Comment))?;
        api::verify_version(comment_version, body.version)?;

        let required_rank = if client.id == comment_owner && comment_owner.is_some() {
            state.config.privileges().comment_edit_own
        } else {
            state.config.privileges().comment_edit_any
        };
        api::verify_privilege(client, required_rank)?;

        diesel::update(comment::table.find(comment_id))
            .set((comment::text.eq(body.text), comment::last_edit_time.eq(DateTime::now())))
            .execute(conn)
            .map_err(ApiError::from)
    })?;
    conn.transaction(|conn| CommentInfo::new_from_id(conn, &state.config, client, comment_id, &fields))
        .map(Json)
        .map_err(ApiError::from)
}

/// See [rating-comment](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#rating-comment)
async fn rate(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(comment_id): Path<i64>,
    Query(params): Query<ResourceParams>,
    Json(body): Json<RatingBody>,
) -> ApiResult<Json<CommentInfo>> {
    api::verify_privilege(client, state.config.privileges().comment_score)?;

    let user_id = client.id.ok_or(ApiError::NotLoggedIn)?;
    let fields = resource::create_table(params.fields()).map_err(Box::from)?;

    let mut conn = state.get_connection()?;
    conn.transaction(|conn| {
        diesel::delete(comment_score::table.find((comment_id, user_id))).execute(conn)?;

        if let Ok(score) = Score::try_from(*body) {
            let insert_result = NewCommentScore {
                comment_id,
                user_id,
                score,
            }
            .insert_into(comment_score::table)
            .execute(conn);
            error::map_foreign_key_violation(insert_result, ResourceType::Comment)?;
        }
        Ok::<_, ApiError>(())
    })?;
    conn.transaction(|conn| CommentInfo::new_from_id(conn, &state.config, client, comment_id, &fields))
        .map(Json)
        .map_err(ApiError::from)
}

/// See [deleting-comment](https://github.com/liamw1/oxibooru/blob/master/doc/API.md#deleting-comment)
async fn delete(
    State(state): State<AppState>,
    Extension(client): Extension<Client>,
    Path(comment_id): Path<i64>,
    Json(client_version): Json<DeleteBody>,
) -> ApiResult<Json<()>> {
    state.get_connection()?.transaction(|conn| {
        let (comment_owner, comment_version): (Option<i64>, DateTime) = comment::table
            .find(comment_id)
            .select((comment::user_id, comment::last_edit_time))
            .first(conn)
            .optional()?
            .ok_or(ApiError::NotFound(ResourceType::Comment))?;
        api::verify_version(comment_version, *client_version)?;

        let required_rank = if client.id == comment_owner && comment_owner.is_some() {
            state.config.privileges().comment_delete_own
        } else {
            state.config.privileges().comment_delete_any
        };
        api::verify_privilege(client, required_rank)?;

        diesel::delete(comment::table.find(comment_id)).execute(conn)?;
        Ok(Json(()))
    })
}

#[cfg(test)]
mod test {
    use crate::api::error::ApiResult;
    use crate::model::comment::Comment;
    use crate::model::enums::{ResourceType, UserRank};
    use crate::schema::{comment, comment_statistics, database_statistics, user, user_statistics};
    use crate::test::*;
    use crate::time::DateTime;
    use diesel::dsl::exists;
    use diesel::{ExpressionMethods, PgConnection, QueryDsl, QueryResult, RunQueryDsl, SelectableHelper};
    use serial_test::{parallel, serial};

    // Exclude fields that involve creation_time or last_edit_time
    const FIELDS: &str = "&fields=id,postId,text,user,score,ownScore";

    #[tokio::test]
    #[parallel]
    async fn list() -> ApiResult<()> {
        const QUERY: &str = "GET /comments/?query";
        const SORT: &str = "-sort:id&limit=40";
        verify_response(&format!("{QUERY}={SORT}{FIELDS}"), "comment/list").await?;
        verify_response(&format!("{QUERY}=sort:score&limit=1{FIELDS}"), "comment/list_highest_score").await?;
        verify_response(&format!("{QUERY}=user:regular_user {SORT}{FIELDS}"), "comment/list_regular_user").await?;
        verify_response(&format!("{QUERY}=text:*this* {SORT}{FIELDS}"), "comment/list_text_filter").await
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

        verify_response(&format!("GET /comment/{COMMENT_ID}/?{FIELDS}"), "comment/get").await?;

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

        verify_response(&format!("POST /comments/?{FIELDS}"), "comment/create").await?;

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

        verify_response(&format!("DELETE /comment/{comment_id}"), "comment/delete").await?;

        let (new_comment_count, new_admin_comment_count) = get_comment_counts(&mut conn)?;
        let has_comment: bool = diesel::select(exists(comment::table.find(comment_id))).first(&mut conn)?;
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

        verify_response(&format!("PUT /comment/{COMMENT_ID}/?{FIELDS}"), "comment/edit").await?;

        let (new_comment, new_score) = get_comment_info(&mut conn)?;
        assert_ne!(new_comment.text, comment.text);
        assert_eq!(new_comment.creation_time, comment.creation_time);
        assert!(new_comment.last_edit_time > comment.last_edit_time);
        assert_eq!(new_score, score);

        verify_response(&format!("PUT /comment/{COMMENT_ID}/?{FIELDS}"), "comment/edit_restore").await?;

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

        verify_response(&format!("PUT /comment/{COMMENT_ID}/score/?{FIELDS}"), "comment/like").await?;

        let (new_score, new_last_edit_time) = get_comment_info(&mut conn)?;
        assert_eq!(new_score, score + 1);
        assert_eq!(new_last_edit_time, last_edit_time);

        verify_response(&format!("PUT /comment/{COMMENT_ID}/score/?{FIELDS}"), "comment/dislike").await?;

        let (new_score, new_last_edit_time) = get_comment_info(&mut conn)?;
        assert_eq!(new_score, score - 1);
        assert_eq!(new_last_edit_time, last_edit_time);

        verify_response(&format!("PUT /comment/{COMMENT_ID}/score/?{FIELDS}"), "comment/remove_score").await?;

        let (new_score, new_last_edit_time) = get_comment_info(&mut conn)?;
        assert_eq!(new_score, score);
        assert_eq!(new_last_edit_time, last_edit_time);
        Ok(())
    }

    #[tokio::test]
    #[parallel]
    async fn error() -> ApiResult<()> {
        verify_response("GET /comment/99", "comment/get_nonexistent").await?;
        verify_response("POST /comments", "comment/create_on_nonexistent_post").await?;
        verify_response("PUT /comment/99", "comment/edit_nonexistent").await?;
        verify_response("PUT /comment/99/score", "comment/like_nonexistent").await?;
        verify_response("DELETE /comment/99", "comment/delete_nonexistent").await?;

        verify_response("PUT /comment/1/score", "comment/invalid_rating").await?;
        verify_response_with_user(UserRank::Anonymous, "PUT /comment/1/score", "comment/rating_anonymously").await?;

        // User has permission to delete own comment, but not another's
        verify_response_with_user(UserRank::Regular, "DELETE /comment/2", "comment/delete_another").await?;

        reset_sequence(ResourceType::Comment)?;
        Ok(())
    }
}
