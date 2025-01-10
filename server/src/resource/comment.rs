use crate::model::comment::{Comment, CommentScore};
use crate::model::enums::{AvatarStyle, Rating};
use crate::resource;
use crate::resource::user::MicroUser;
use crate::schema::{comment, comment_score, comment_statistics, user};
use crate::time::DateTime;
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
    pub score: i32,
    pub own_score: Rating,
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
        let comments = resource::order_as(unordered_posts, &comment_ids);
        Self::new_batch(conn, client, comments)
    }
}

fn get_owners(conn: &mut PgConnection, comments: &[Comment]) -> QueryResult<Vec<Option<MicroUser>>> {
    let comment_ids: Vec<_> = comments.iter().map(Identifiable::id).copied().collect();
    comment::table
        .filter(comment::id.eq_any(&comment_ids))
        .inner_join(user::table)
        .select((comment::id, user::name, user::avatar_style))
        .load::<(i32, String, AvatarStyle)>(conn)
        .map(|comment_info| {
            resource::order_like(comment_info, comments, |&(id, ..)| id)
                .into_iter()
                .map(|comment_owner| {
                    comment_owner.map(|(_, username, avatar_style)| MicroUser::new(username, avatar_style))
                })
                .collect()
        })
}

fn get_scores(conn: &mut PgConnection, comments: &[Comment]) -> QueryResult<Vec<i32>> {
    let comment_ids: Vec<_> = comments.iter().map(Identifiable::id).copied().collect();
    comment_statistics::table
        .select((comment_statistics::comment_id, comment_statistics::score))
        .filter(comment_statistics::comment_id.eq_any(&comment_ids))
        .load(conn)
        .map(|comment_scores| {
            resource::order_transformed_as(comment_scores, &comment_ids, |&(id, _)| id)
                .into_iter()
                .map(|(_, score)| score)
                .collect()
        })
}

fn get_client_scores(conn: &mut PgConnection, client: Option<i32>, comments: &[Comment]) -> QueryResult<Vec<Rating>> {
    if let Some(client_id) = client {
        CommentScore::belonging_to(comments)
            .filter(comment_score::comment_id.eq(client_id))
            .load::<CommentScore>(conn)
            .map(|client_scores| {
                resource::order_like(client_scores, comments, |score| score.comment_id)
                    .into_iter()
                    .map(|client_score| client_score.map(|score| Rating::from(score.score)).unwrap_or_default())
                    .collect()
            })
    } else {
        Ok(vec![Rating::default(); comments.len()])
    }
}
