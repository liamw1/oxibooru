use crate::model::comment::Comment;
use crate::model::enums::{AvatarStyle, Rating, Score};
use crate::model::IntegerIdentifiable;
use crate::resource;
use crate::resource::user::MicroUser;
use crate::schema::{comment, comment_score, user};
use crate::time::DateTime;
use diesel::dsl::*;
use diesel::prelude::*;
use serde::Serialize;
use std::collections::HashMap;

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
    pub user: MicroUser,
    pub score: i64,
    pub own_score: Rating,
}

impl IntegerIdentifiable for CommentInfo {
    fn id(&self) -> i32 {
        self.id
    }
}

impl CommentInfo {
    pub fn new_from_id(conn: &mut PgConnection, client: Option<i32>, comment_id: i32) -> QueryResult<Self> {
        let mut comment_info = Self::new_batch_from_ids(conn, client, vec![comment_id])?;
        assert_eq!(comment_info.len(), 1);
        Ok(comment_info.pop().unwrap())
    }

    pub fn new_batch_from_ids(
        conn: &mut PgConnection,
        client: Option<i32>,
        comment_ids: Vec<i32>,
    ) -> QueryResult<Vec<Self>> {
        let comments: Vec<(Comment, String, AvatarStyle)> = comment::table
            .inner_join(user::table)
            .select((Comment::as_select(), user::name, user::avatar_style))
            .filter(comment::id.eq_any(&comment_ids))
            .load(conn)?;
        let scores: HashMap<i32, Option<i64>> = comment_score::table
            .group_by(comment_score::comment_id)
            .select((comment_score::comment_id, sum(comment_score::score)))
            .filter(comment_score::comment_id.eq_any(&comment_ids))
            .load(conn)?
            .into_iter()
            .collect();
        let client_scores: HashMap<i32, Score> = client
            .map(|user_id| {
                comment_score::table
                    .select((comment_score::comment_id, comment_score::score))
                    .filter(comment_score::comment_id.eq_any(&comment_ids))
                    .filter(comment_score::user_id.eq(user_id))
                    .load(conn)
            })
            .transpose()?
            .unwrap_or_default()
            .into_iter()
            .collect();

        let comment_info = comments
            .into_iter()
            .map(|(comment, author, avatar_style)| Self {
                version: comment.last_edit_time,
                id: comment.id,
                post_id: comment.post_id,
                text: comment.text,
                creation_time: comment.creation_time,
                last_edit_time: comment.last_edit_time,
                user: MicroUser::new(author, avatar_style),
                score: scores.get(&comment.id).copied().flatten().unwrap_or(0),
                own_score: client_scores
                    .get(&comment.id)
                    .copied()
                    .map(Rating::from)
                    .unwrap_or_default(),
            })
            .collect();
        Ok(resource::order_by(comment_info, &comment_ids))
    }
}
