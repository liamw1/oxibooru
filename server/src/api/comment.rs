use crate::api::micro::MicroUser;
use crate::model::comment::Comment;
use crate::model::user::User;
use crate::schema::{comment_score, user};
use crate::util::DateTime;
use diesel::prelude::*;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentInfo {
    version: DateTime, // TODO: Remove last_edit_time as it fills the same role as version here
    id: i32,
    post_id: i32,
    user: MicroUser,
    text: String,
    creation_time: DateTime,
    last_edit_time: DateTime,
    score: i64,
    own_score: Option<i32>,
}

impl CommentInfo {
    pub fn new(conn: &mut PgConnection, comment: Comment, client: Option<i32>) -> QueryResult<Self> {
        let owner = user::table
            .find(comment.user_id)
            .select(User::as_select())
            .first(conn)?;
        let score = comment.score(conn)?;
        let client_score = client
            .map(|id| {
                comment_score::table
                    .find((comment.id, id))
                    .select(comment_score::score)
                    .first(conn)
                    .optional()
            })
            .transpose()?;

        Ok(CommentInfo {
            version: comment.last_edit_time.clone().into(),
            id: comment.id,
            post_id: comment.post_id,
            user: MicroUser::new(owner),
            text: comment.text,
            creation_time: comment.creation_time,
            last_edit_time: comment.last_edit_time,
            score,
            own_score: client_score.flatten(),
        })
    }
}
