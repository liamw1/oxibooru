use crate::model::user::UserToken;
use crate::resource::user::MicroUser;
use crate::util::DateTime;
use serde::Serialize;
use uuid::Uuid;

// TODO: Remove renames by changing references to these names in client
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserTokenInfo {
    version: DateTime, // TODO: Remove last_edit_time as it fills the same role as version here
    user: MicroUser,
    token: Uuid,
    note: String,
    enabled: bool,
    expiration_time: Option<DateTime>,
    creation_time: DateTime,
    last_edit_time: DateTime,
    last_usage_time: DateTime,
}

impl UserTokenInfo {
    pub fn new(user: MicroUser, user_token: UserToken) -> Self {
        UserTokenInfo {
            version: user_token.last_edit_time,
            user,
            token: user_token.token,
            note: user_token.note,
            enabled: user_token.enabled,
            expiration_time: user_token.expiration_time,
            creation_time: user_token.creation_time,
            last_edit_time: user_token.last_edit_time,
            last_usage_time: user_token.last_usage_time,
        }
    }
}
