use crate::model::user::UserToken;
use crate::resource::field::Mask;
use crate::resource::user::MicroUser;
use crate::string::LargeString;
use crate::time::DateTime;
use serde::Serialize;
use server_macros::resource;
use strum::EnumString;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Clone, Copy, EnumString)]
#[strum(serialize_all = "camelCase")]
pub enum Field {
    Version,
    User,
    Token,
    Note,
    Enabled,
    ExpirationTime,
    CreationTime,
    LastEditTime,
    LastUsageTime,
}

impl From<Field> for u64 {
    fn from(value: Field) -> Self {
        value as u64
    }
}

/// A single user token.
#[resource]
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserTokenInfo {
    /// Resource version. See [versioning](#Versioning).
    pub version: DateTime,
    /// The user that owns the token.
    pub user: MicroUser,
    /// The token that can be used to authenticate the user.
    pub token: Uuid,
    /// A note that describes the token.
    pub note: LargeString,
    /// Whether the token is still valid for authentication.
    pub enabled: bool,
    /// Time when the token expires.
    pub expiration_time: Option<DateTime>,
    /// Time the user token was created.
    pub creation_time: DateTime,
    /// Time the user token was last edited.
    pub last_edit_time: DateTime,
    /// The last time this token was used during a login involving `?bump-login`.
    pub last_usage_time: DateTime,
}

impl UserTokenInfo {
    pub fn new(user: MicroUser, user_token: UserToken, fields: Mask<Field>) -> Self {
        UserTokenInfo {
            version: fields[Field::Version].then_some(user_token.last_edit_time),
            user: fields[Field::User].then_some(user),
            token: fields[Field::Token].then_some(user_token.id),
            note: fields[Field::Note].then_some(user_token.note),
            enabled: fields[Field::Enabled].then_some(user_token.enabled),
            expiration_time: fields[Field::ExpirationTime].then_some(user_token.expiration_time),
            creation_time: fields[Field::CreationTime].then_some(user_token.creation_time),
            last_edit_time: fields[Field::LastEditTime].then_some(user_token.last_edit_time),
            last_usage_time: fields[Field::LastUsageTime].then_some(user_token.last_usage_time),
        }
    }
}
