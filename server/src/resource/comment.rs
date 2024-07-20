use crate::resource::user::MicroUser;
use crate::util::DateTime;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentInfo {
    pub version: DateTime, // TODO: Remove last_edit_time as it fills the same role as version here
    pub id: i32,
    pub post_id: i32,
    pub user: MicroUser,
    pub text: String,
    pub creation_time: DateTime,
    pub last_edit_time: DateTime,
    pub score: i64,
    pub own_score: Option<i32>,
}
