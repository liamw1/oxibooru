use crate::schema::comment;
use crate::schema::comment_score;
use chrono::DateTime;
use chrono::Utc;
use diesel::prelude::*;

#[derive(Insertable)]
#[diesel(table_name = comment)]
pub struct NewComment<'a> {
    pub user_id: i32,
    pub post_id: i32,
    pub text: &'a str,
    pub creation_time: DateTime<Utc>,
    pub last_edit_time: DateTime<Utc>,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = comment)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Comment {
    pub id: i32,
    pub user_id: i32,
    pub post_id: i32,
    pub text: String,
    pub creation_time: DateTime<Utc>,
    pub last_edit_time: DateTime<Utc>,
}

#[allow(dead_code)]
type NewCommentScore = CommentScore;

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = comment_score)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct CommentScore {
    pub comment_id: i32,
    pub user_id: i32,
    pub score: i32,
    pub time: DateTime<Utc>,
}