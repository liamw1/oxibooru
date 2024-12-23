use crate::model::comment::{Comment, CommentScore};
use crate::model::enums::{AvatarStyle, Rating};
use crate::model::IntegerIdentifiable;
use crate::resource;
use crate::resource::user::MicroUser;
use crate::schema::{comment, comment_score, user};
use crate::time::DateTime;
use diesel::dsl::*;
use diesel::prelude::*;
use serde::Serialize;

/// No field selecting for comments

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentInfo {
    pub version: DateTime, // TODO: Remove last_edit_time as it fills the same role as version here
    pub id: i32,
    pub post_id: i32,
    pub text: String,
    pub creation_time: DateTime,
    pub last_edit_time: DateTime,
    pub user: Option<MicroUser>,
    pub score: i64,
    pub own_score: Rating,
}

impl IntegerIdentifiable for CommentInfo {
    fn id(&self) -> i32 {
        self.id
    }
}

impl CommentInfo {
    pub fn new(conn: &mut PgConnection, client: Option<i32>, comment: Comment) -> QueryResult<Self> {
        let mut comment_info = Self::new_batch(conn, client, vec![comment])?;
        assert_eq!(comment_info.len(), 1);
        Ok(comment_info.pop().unwrap())
    }

    pub fn new_from_id(conn: &mut PgConnection, client: Option<i32>, comment_id: i32) -> QueryResult<Self> {
        let mut comment_info = Self::new_batch_from_ids(conn, client, vec![comment_id])?;
        assert_eq!(comment_info.len(), 1);
        Ok(comment_info.pop().unwrap())
    }

    pub fn new_batch(conn: &mut PgConnection, client: Option<i32>, comments: Vec<Comment>) -> QueryResult<Vec<Self>> {
        let batch_size = comments.len();

        let mut owners = get_owners(conn, &comments)?;
        resource::check_batch_results(owners.len(), batch_size);

        let mut scores = get_scores(conn, &comments)?;
        resource::check_batch_results(scores.len(), batch_size);

        let mut client_scores = get_client_scores(conn, client, &comments)?;
        resource::check_batch_results(client_scores.len(), batch_size);

        let results = comments
            .into_iter()
            .rev()
            .map(|comment| Self {
                version: comment.last_edit_time,
                id: comment.id,
                post_id: comment.post_id,
                text: comment.text,
                creation_time: comment.creation_time,
                last_edit_time: comment.last_edit_time,
                user: owners.pop().flatten(),
                score: scores.pop().unwrap_or(0),
                own_score: client_scores.pop().unwrap_or_default(),
            })
            .collect::<Vec<_>>();
        Ok(results.into_iter().rev().collect())
    }

    pub fn new_batch_from_ids(
        conn: &mut PgConnection,
        client: Option<i32>,
        comment_ids: Vec<i32>,
    ) -> QueryResult<Vec<Self>> {
        let unordered_posts = comment::table.filter(comment::id.eq_any(&comment_ids)).load(conn)?;
        let comments = resource::order_by(unordered_posts, &comment_ids);
        Self::new_batch(conn, client, comments)
    }
}

fn get_owners(conn: &mut PgConnection, comments: &[Comment]) -> QueryResult<Vec<Option<MicroUser>>> {
    let comment_ids = comments.iter().map(|comment| comment.id).collect::<Vec<_>>();
    comment::table
        .filter(comment::id.eq_any(&comment_ids))
        .inner_join(user::table)
        .select((comment::id, user::name, user::avatar_style))
        .load::<(i32, String, AvatarStyle)>(conn)
        .map(|comment_info| {
            resource::order_as(comment_info, comments, |(id, ..)| *id)
                .into_iter()
                .map(|comment_owner| {
                    comment_owner.map(|(_, username, avatar_style)| MicroUser::new(username, avatar_style))
                })
                .collect()
        })
}

fn get_scores(conn: &mut PgConnection, comments: &[Comment]) -> QueryResult<Vec<i64>> {
    CommentScore::belonging_to(comments)
        .group_by(comment_score::comment_id)
        .select((comment_score::comment_id, sum(comment_score::score)))
        .load(conn)
        .map(|comment_scores| {
            resource::order_as(comment_scores, comments, |(id, _)| *id)
                .into_iter()
                .map(|comment_score| comment_score.and_then(|(_, score)| score).unwrap_or(0))
                .collect()
        })
}

fn get_client_scores(conn: &mut PgConnection, client: Option<i32>, comments: &[Comment]) -> QueryResult<Vec<Rating>> {
    if let Some(client_id) = client {
        CommentScore::belonging_to(comments)
            .filter(comment_score::comment_id.eq(client_id))
            .load::<CommentScore>(conn)
            .map(|client_scores| {
                resource::order_as(client_scores, comments, |score| score.comment_id)
                    .into_iter()
                    .map(|client_score| client_score.map(|score| Rating::from(score.score)).unwrap_or_default())
                    .collect()
            })
    } else {
        Ok(vec![Rating::default(); comments.len()])
    }
}
