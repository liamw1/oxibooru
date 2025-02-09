use crate::model::user::UserToken;
use crate::resource::user::MicroUser;
use crate::time::DateTime;
use serde::Serialize;
use serde_with::skip_serializing_none;
use std::str::FromStr;
use strum::{EnumString, EnumTable};
use uuid::Uuid;

#[derive(Clone, Copy, EnumString, EnumTable)]
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

impl Field {
    pub fn create_table(fields: Option<&str>) -> Result<FieldTable<bool>, <Self as FromStr>::Err> {
        if let Some(fields_str) = fields {
            let mut table = FieldTable::filled(false);
            for field in fields_str.split(',') {
                table[Self::from_str(field)?] = true;
            }
            Ok(table)
        } else {
            Ok(FieldTable::filled(true))
        }
    }
}

#[skip_serializing_none]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserTokenInfo {
    version: Option<DateTime>,
    user: Option<MicroUser>,
    token: Option<Uuid>,
    note: Option<String>,
    enabled: Option<bool>,
    expiration_time: Option<Option<DateTime>>,
    creation_time: Option<DateTime>,
    last_edit_time: Option<DateTime>,
    last_usage_time: Option<DateTime>,
}

impl UserTokenInfo {
    pub fn new(user: MicroUser, user_token: UserToken, fields: &FieldTable<bool>) -> Self {
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
