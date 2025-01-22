use crate::model::enums::Score;
use crate::model::post::Post;
use crate::model::user::User;
use crate::schema::{comment, comment_score};
use crate::time::DateTime;
use diesel::pg::Pg;
use diesel::prelude::*;

#[derive(Insertable)]
#[diesel(table_name = comment)]
#[diesel(check_for_backend(Pg))]
pub struct NewComment<'a> {
    pub user_id: Option<i64>,
    pub post_id: i64,
    pub text: &'a str,
    pub creation_time: DateTime,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(User), belongs_to(Post))]
#[diesel(table_name = comment)]
#[diesel(check_for_backend(Pg))]
pub struct Comment {
    pub id: i64,
    pub user_id: Option<i64>,
    pub post_id: i64,
    pub text: String,
    pub creation_time: DateTime,
    pub last_edit_time: DateTime,
}

#[derive(Insertable)]
#[diesel(table_name = comment_score)]
#[diesel(check_for_backend(Pg))]
pub struct NewCommentScore {
    pub comment_id: i64,
    pub user_id: i64,
    pub score: Score,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(Comment), belongs_to(User))]
#[diesel(table_name = comment_score)]
#[diesel(primary_key(comment_id, user_id))]
#[diesel(check_for_backend(Pg))]
pub struct CommentScore {
    pub comment_id: i64,
    pub user_id: i64,
    pub score: Score,
    pub time: DateTime,
}
