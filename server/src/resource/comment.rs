use crate::auth::Client;
use crate::model::comment::{Comment, CommentScore};
use crate::model::enums::{AvatarStyle, Rating};
use crate::resource::user::MicroUser;
use crate::resource::{self, BoolFill};
use crate::schema::{comment, comment_score, comment_statistics, user};
use crate::string::{LargeString, SmallString};
use crate::time::DateTime;
use diesel::prelude::*;
use serde::Serialize;
use serde_with::skip_serializing_none;
use strum::{EnumString, EnumTable};

#[derive(Clone, Copy, EnumString, EnumTable)]
#[strum(serialize_all = "camelCase")]
pub enum Field {
    Version,
    Id,
    PostId,
    Text,
    CreationTime,
    LastEditTime,
    User,
    Score,
    OwnScore,
}

impl BoolFill for FieldTable<bool> {
    fn filled(val: bool) -> Self {
        Self::filled(val)
    }
}

#[skip_serializing_none]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentInfo {
    pub version: Option<DateTime>, // TODO: Remove last_edit_time as it fills the same role as version here
    pub id: Option<i64>,
    pub post_id: Option<i64>,
    pub text: Option<LargeString>,
    pub creation_time: Option<DateTime>,
    pub last_edit_time: Option<DateTime>,
    pub user: Option<Option<MicroUser>>,
    pub score: Option<i64>,
    pub own_score: Option<Rating>,
}

impl CommentInfo {
    pub fn new(
        conn: &mut PgConnection,
        client: Client,
        comment: Comment,
        fields: &FieldTable<bool>,
    ) -> QueryResult<Self> {
        Self::new_batch(conn, client, vec![comment], fields).map(resource::single)
    }

    pub fn new_from_id(
        conn: &mut PgConnection,
        client: Client,
        comment_id: i64,
        fields: &FieldTable<bool>,
    ) -> QueryResult<Self> {
        Self::new_batch_from_ids(conn, client, &[comment_id], fields).map(resource::single)
    }

    pub fn new_batch(
        conn: &mut PgConnection,
        client: Client,
        comments: Vec<Comment>,
        fields: &FieldTable<bool>,
    ) -> QueryResult<Vec<Self>> {
        let mut owners = resource::retrieve(fields[Field::User], || get_owners(conn, &comments))?;
        let mut scores = resource::retrieve(fields[Field::Score], || get_scores(conn, &comments))?;
        let mut client_scores =
            resource::retrieve(fields[Field::OwnScore], || get_client_scores(conn, client, &comments))?;

        let batch_size = comments.len();
        resource::check_batch_results(batch_size, owners.len());
        resource::check_batch_results(batch_size, scores.len());
        resource::check_batch_results(batch_size, client_scores.len());

        let results = comments
            .into_iter()
            .rev()
            .map(|comment| Self {
                version: fields[Field::Version].then_some(comment.last_edit_time),
                id: fields[Field::Id].then_some(comment.id),
                post_id: fields[Field::PostId].then_some(comment.post_id),
                text: fields[Field::Text].then_some(comment.text),
                creation_time: fields[Field::CreationTime].then_some(comment.creation_time),
                last_edit_time: fields[Field::LastEditTime].then_some(comment.last_edit_time),
                user: owners.pop(),
                score: scores.pop(),
                own_score: client_scores.pop(),
            })
            .collect::<Vec<_>>();
        Ok(results.into_iter().rev().collect())
    }

    pub fn new_batch_from_ids(
        conn: &mut PgConnection,
        client: Client,
        comment_ids: &[i64],
        fields: &FieldTable<bool>,
    ) -> QueryResult<Vec<Self>> {
        let unordered_comments = comment::table.filter(comment::id.eq_any(comment_ids)).load(conn)?;
        let comments = resource::order_as(unordered_comments, comment_ids);
        Self::new_batch(conn, client, comments, fields)
    }
}

fn get_owners(conn: &mut PgConnection, comments: &[Comment]) -> QueryResult<Vec<Option<MicroUser>>> {
    let comment_ids: Vec<_> = comments.iter().map(Identifiable::id).copied().collect();
    comment::table
        .filter(comment::id.eq_any(&comment_ids))
        .inner_join(user::table)
        .select((comment::id, user::name, user::avatar_style))
        .load::<(i64, SmallString, AvatarStyle)>(conn)
        .map(|comment_info| {
            resource::order_like(comment_info, comments, |&(id, ..)| id)
                .into_iter()
                .map(|comment_owner| {
                    comment_owner.map(|(_, username, avatar_style)| MicroUser::new(username, avatar_style))
                })
                .collect()
        })
}

fn get_scores(conn: &mut PgConnection, comments: &[Comment]) -> QueryResult<Vec<i64>> {
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

fn get_client_scores(conn: &mut PgConnection, client: Client, comments: &[Comment]) -> QueryResult<Vec<Rating>> {
    if let Some(client_id) = client.id {
        CommentScore::belonging_to(comments)
            .filter(comment_score::user_id.eq(client_id))
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
