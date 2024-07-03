use crate::model::enums::UserRank;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Deserialize)]
pub struct Thumbnails {
    pub avatar_width: u32,
    pub avatar_height: u32,
    pub post_width: u32,
    pub post_height: u32,
}

/*
    Stores user rank required for most actions a client could take.
    TODO: Remove renames (will require modifying client)
*/
#[derive(Serialize, Deserialize)]
pub struct Privileges {
    #[serde(rename(serialize = "users:create:self"))]
    pub user_create_self: UserRank,
    #[serde(rename(serialize = "users:create:any"))]
    pub user_create_any: UserRank,
    #[serde(rename(serialize = "users:list"))]
    pub user_list: UserRank,
    #[serde(rename(serialize = "users:view"))]
    pub user_view: UserRank,
    #[serde(rename(serialize = "users:edit:any:name"))]
    pub user_edit_any_name: UserRank,
    #[serde(rename(serialize = "users:edit:any:pass"))]
    pub user_edit_any_pass: UserRank,
    #[serde(rename(serialize = "users:edit:any:email"))]
    pub user_edit_any_email: UserRank,
    #[serde(rename(serialize = "users:edit:any:avatar"))]
    pub user_edit_any_avatar: UserRank,
    #[serde(rename(serialize = "users:edit:any:rank"))]
    pub user_edit_any_rank: UserRank,
    #[serde(rename(serialize = "users:edit:self:name"))]
    pub user_edit_self_name: UserRank,
    #[serde(rename(serialize = "users:edit:self:pass"))]
    pub user_edit_self_pass: UserRank,
    #[serde(rename(serialize = "users:edit:self:email"))]
    pub user_edit_self_email: UserRank,
    #[serde(rename(serialize = "users:edit:self:avatar"))]
    pub user_edit_self_avatar: UserRank,
    #[serde(rename(serialize = "users:edit:self:rank"))]
    pub user_edit_self_rank: UserRank,
    #[serde(rename(serialize = "users:delete:any"))]
    pub user_delete_any: UserRank,
    #[serde(rename(serialize = "users:delete:self"))]
    pub user_delete_self: UserRank,

    #[serde(rename(serialize = "userTokens:list:any"))]
    pub user_token_list_any: UserRank,
    #[serde(rename(serialize = "userTokens:list:self"))]
    pub user_token_list_self: UserRank,
    #[serde(rename(serialize = "userTokens:create:any"))]
    pub user_token_create_any: UserRank,
    #[serde(rename(serialize = "userTokens:create:self"))]
    pub user_token_create_self: UserRank,
    #[serde(rename(serialize = "userTokens:edit:any"))]
    pub user_token_edit_any: UserRank,
    #[serde(rename(serialize = "userTokens:edit:self"))]
    pub user_token_edit_self: UserRank,
    #[serde(rename(serialize = "userTokens:delete:any"))]
    pub user_token_delete_any: UserRank,
    #[serde(rename(serialize = "userTokens:delete:self"))]
    pub user_token_delete_self: UserRank,

    #[serde(rename(serialize = "posts:create:anonymous"))]
    pub post_create_anonymous: UserRank,
    #[serde(rename(serialize = "posts:create:identified"))]
    pub post_create_identified: UserRank,
    #[serde(rename(serialize = "posts:list"))]
    pub post_list: UserRank,
    #[serde(rename(serialize = "posts:reverseSearch"))]
    pub post_reverse_search: UserRank,
    #[serde(rename(serialize = "posts:view"))]
    pub post_view: UserRank,
    #[serde(rename(serialize = "posts:view:featured"))]
    pub post_view_featured: UserRank,
    #[serde(rename(serialize = "posts:edit:content"))]
    pub post_edit_content: UserRank,
    #[serde(rename(serialize = "posts:edit:flags"))]
    pub post_edit_flag: UserRank,
    #[serde(rename(serialize = "posts:edit:notes"))]
    pub post_edit_note: UserRank,
    #[serde(rename(serialize = "posts:edit:relations"))]
    pub post_edit_relation: UserRank,
    #[serde(rename(serialize = "posts:edit:safety"))]
    pub post_edit_safety: UserRank,
    #[serde(rename(serialize = "posts:edit:source"))]
    pub post_edit_source: UserRank,
    #[serde(rename(serialize = "posts:edit:tags"))]
    pub post_edit_tag: UserRank,
    #[serde(rename(serialize = "posts:edit:thumbnail"))]
    pub post_edit_thumbnail: UserRank,
    #[serde(rename(serialize = "posts:feature"))]
    pub post_feature: UserRank,
    #[serde(rename(serialize = "posts:delete"))]
    pub post_delete: UserRank,
    #[serde(rename(serialize = "posts:score"))]
    pub post_score: UserRank,
    #[serde(rename(serialize = "posts:merge"))]
    pub post_merge: UserRank,
    #[serde(rename(serialize = "posts:favorite"))]
    pub post_favorite: UserRank,
    #[serde(rename(serialize = "posts:bulk-edit:tags"))]
    pub post_bulk_edit_tag: UserRank,
    #[serde(rename(serialize = "posts:bulk-edit:safety"))]
    pub post_bulk_edit_safety: UserRank,
    #[serde(rename(serialize = "posts:bulk-edit:delete"))]
    pub post_bulk_edit_delete: UserRank,

    #[serde(rename(serialize = "tags:create"))]
    pub tag_create: UserRank,
    #[serde(rename(serialize = "tags:edit:names"))]
    pub tag_edit_name: UserRank,
    #[serde(rename(serialize = "tags:edit:category"))]
    pub tag_edit_category: UserRank,
    #[serde(rename(serialize = "tags:edit:description"))]
    pub tag_edit_description: UserRank,
    #[serde(rename(serialize = "tags:edit:implications"))]
    pub tag_edit_implication: UserRank,
    #[serde(rename(serialize = "tags:edit:suggestions"))]
    pub tag_edit_suggestion: UserRank,
    #[serde(rename(serialize = "tags:list"))]
    pub tag_list: UserRank,
    #[serde(rename(serialize = "tags:view"))]
    pub tag_view: UserRank,
    #[serde(rename(serialize = "tags:merge"))]
    pub tag_merge: UserRank,
    #[serde(rename(serialize = "tags:delete"))]
    pub tag_delete: UserRank,

    #[serde(rename(serialize = "tagCategories:create"))]
    pub tag_category_create: UserRank,
    #[serde(rename(serialize = "tagCategories:edit:name"))]
    pub tag_category_edit_name: UserRank,
    #[serde(rename(serialize = "tagCategories:edit:color"))]
    pub tag_category_edit_color: UserRank,
    #[serde(rename(serialize = "tagCategories:edit:order"))]
    pub tag_category_edit_order: UserRank,
    #[serde(rename(serialize = "tagCategories:list"))]
    pub tag_category_list: UserRank,
    #[serde(rename(serialize = "tagCategories:view"))]
    pub tag_category_view: UserRank,
    #[serde(rename(serialize = "tagCategories:delete"))]
    pub tag_category_delete: UserRank,
    #[serde(rename(serialize = "tagCategories:setDefault"))]
    pub tag_category_set_default: UserRank,

    #[serde(rename(serialize = "pools:create"))]
    pub pool_create: UserRank,
    #[serde(rename(serialize = "pools:edit:names"))]
    pub pool_edit_name: UserRank,
    #[serde(rename(serialize = "pools:edit:category"))]
    pub pool_edit_category: UserRank,
    #[serde(rename(serialize = "pools:edit:description"))]
    pub pool_edit_description: UserRank,
    #[serde(rename(serialize = "pools:edit:posts"))]
    pub pool_edit_post: UserRank,
    #[serde(rename(serialize = "pools:list"))]
    pub pool_list: UserRank,
    #[serde(rename(serialize = "pools:view"))]
    pub pool_view: UserRank,
    #[serde(rename(serialize = "pools:merge"))]
    pub pool_merge: UserRank,
    #[serde(rename(serialize = "pools:delete"))]
    pub pool_delete: UserRank,

    #[serde(rename(serialize = "poolCategories:create"))]
    pub pool_category_create: UserRank,
    #[serde(rename(serialize = "poolCategories:edit:name"))]
    pub pool_category_edit_name: UserRank,
    #[serde(rename(serialize = "poolCategories:edit:color"))]
    pub pool_category_edit_color: UserRank,
    #[serde(rename(serialize = "poolCategories:list"))]
    pub pool_category_list: UserRank,
    #[serde(rename(serialize = "poolCategories:view"))]
    pub pool_category_view: UserRank,
    #[serde(rename(serialize = "poolCategories:delete"))]
    pub pool_category_delete: UserRank,
    #[serde(rename(serialize = "poolCategories:setDefault"))]
    pub pool_category_set_default: UserRank,

    #[serde(rename(serialize = "comments:create"))]
    pub comment_create: UserRank,
    #[serde(rename(serialize = "comments:delete:any"))]
    pub comment_delete_any: UserRank,
    #[serde(rename(serialize = "comments:delete:own"))]
    pub comment_delete_own: UserRank,
    #[serde(rename(serialize = "comments:edit:any"))]
    pub comment_edit_any: UserRank,
    #[serde(rename(serialize = "comments:edit:own"))]
    pub comment_edit_own: UserRank,
    #[serde(rename(serialize = "comments:list"))]
    pub comment_list: UserRank,
    #[serde(rename(serialize = "comments:view"))]
    pub comment_view: UserRank,
    #[serde(rename(serialize = "comments:score"))]
    pub comment_score: UserRank,

    #[serde(rename(serialize = "snapshots:list"))]
    pub snapshot_list: UserRank,

    #[serde(rename(serialize = "uploads:create"))]
    pub upload_create: UserRank,
    #[serde(rename(serialize = "uploads:useDownloader"))]
    pub upload_use_downloader: UserRank,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct PublicInfo {
    pub name: String,
    pub default_user_rank: UserRank,
    pub enable_safety: bool,
    pub contact_email: Option<String>,
    #[serde(rename(serialize = "canSendMails"))]
    pub can_send_mail: bool,
    #[serde(rename(serialize = "userNameRegex"))]
    pub username_regex: String,
    pub password_regex: String,
    pub tag_name_regex: String,
    pub tag_category_name_regex: String,
    pub privileges: Privileges,
}

#[derive(Deserialize)]
pub struct Config {
    pub password_secret: String,
    pub content_secret: String,
    pub data_url: String,
    pub data_dir: String,
    pub delete_source_files: bool,
    pub pool_name_regex: String,
    pub pool_category_regex: String,
    pub thumbnails: Thumbnails,
    pub public_info: PublicInfo,
}

pub fn get() -> &'static Config {
    &CONFIG
}

pub fn privileges() -> &'static Privileges {
    &CONFIG.public_info.privileges
}

static CONFIG: Lazy<Config> =
    Lazy::new(|| toml::from_str(&std::fs::read_to_string(get_config_path()).unwrap()).unwrap());

fn get_config_path() -> PathBuf {
    // Use config.toml.dist if in development environment, config.toml if in production
    match std::env::var("CARGO_MANIFEST_DIR") {
        Ok(var) => {
            let mut project_path = PathBuf::from(var);
            project_path.push("config.toml.dist");
            project_path
        }
        Err(_) => {
            let exe_path = std::env::current_exe().unwrap();
            let mut parent_path = exe_path.parent().expect("Exe path should have parent").to_owned();
            parent_path.push("config.toml");
            parent_path
        }
    }
}
