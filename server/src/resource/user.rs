use crate::auth::content;
use crate::model::enums::AvatarStyle;
use crate::model::user::User;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicroUser {
    name: String,
    avatar_url: String,
}

impl MicroUser {
    pub fn new(user: User) -> Self {
        let avatar_url = user.avatar_url();
        Self {
            name: user.name,
            avatar_url,
        }
    }

    pub fn new2(name: String, avatar_style: AvatarStyle) -> Self {
        let avatar_url = match avatar_style {
            AvatarStyle::Gravatar => content::gravatar_url(&name),
            AvatarStyle::Manual => content::custom_avatar_url(&name),
        };
        Self { name, avatar_url }
    }
}
