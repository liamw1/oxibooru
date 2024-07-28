use crate::model::enums::UserRank;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::LazyLock;

#[derive(Deserialize)]
pub struct Thumbnails {
    pub avatar_width: u32,
    pub avatar_height: u32,
    pub post_width: u32,
    pub post_height: u32,
}

#[derive(Serialize, Deserialize)]
pub struct Privileges {
    pub user_create_self: UserRank,
    pub user_create_any: UserRank,
    pub user_list: UserRank,
    pub user_view: UserRank,
    pub user_edit_any_name: UserRank,
    pub user_edit_any_pass: UserRank,
    pub user_edit_any_email: UserRank,
    pub user_edit_any_avatar: UserRank,
    pub user_edit_any_rank: UserRank,
    pub user_edit_self_name: UserRank,
    pub user_edit_self_pass: UserRank,
    pub user_edit_self_email: UserRank,
    pub user_edit_self_avatar: UserRank,
    pub user_edit_self_rank: UserRank,
    pub user_delete_any: UserRank,
    pub user_delete_self: UserRank,

    pub user_token_list_any: UserRank,
    pub user_token_list_self: UserRank,
    pub user_token_create_any: UserRank,
    pub user_token_create_self: UserRank,
    pub user_token_edit_any: UserRank,
    pub user_token_edit_self: UserRank,
    pub user_token_delete_any: UserRank,
    pub user_token_delete_self: UserRank,

    pub post_create_anonymous: UserRank,
    pub post_create_identified: UserRank,
    pub post_list: UserRank,
    pub post_reverse_search: UserRank,
    pub post_view: UserRank,
    pub post_view_featured: UserRank,
    pub post_edit_content: UserRank,
    pub post_edit_flag: UserRank,
    pub post_edit_note: UserRank,
    pub post_edit_relation: UserRank,
    pub post_edit_safety: UserRank,
    pub post_edit_source: UserRank,
    pub post_edit_tag: UserRank,
    pub post_edit_thumbnail: UserRank,
    pub post_feature: UserRank,
    pub post_delete: UserRank,
    pub post_score: UserRank,
    pub post_merge: UserRank,
    pub post_favorite: UserRank,
    pub post_bulk_edit_tag: UserRank,
    pub post_bulk_edit_safety: UserRank,
    pub post_bulk_edit_delete: UserRank,

    pub tag_create: UserRank,
    pub tag_edit_name: UserRank,
    pub tag_edit_category: UserRank,
    pub tag_edit_description: UserRank,
    pub tag_edit_implication: UserRank,
    pub tag_edit_suggestion: UserRank,
    pub tag_list: UserRank,
    pub tag_view: UserRank,
    pub tag_merge: UserRank,
    pub tag_delete: UserRank,

    pub tag_category_create: UserRank,
    pub tag_category_edit_name: UserRank,
    pub tag_category_edit_color: UserRank,
    pub tag_category_edit_order: UserRank,
    pub tag_category_list: UserRank,
    pub tag_category_view: UserRank,
    pub tag_category_delete: UserRank,
    pub tag_category_set_default: UserRank,

    pub pool_create: UserRank,
    pub pool_edit_name: UserRank,
    pub pool_edit_category: UserRank,
    pub pool_edit_description: UserRank,
    pub pool_edit_post: UserRank,
    pub pool_list: UserRank,
    pub pool_view: UserRank,
    pub pool_merge: UserRank,
    pub pool_delete: UserRank,

    pub pool_category_create: UserRank,
    pub pool_category_edit_name: UserRank,
    pub pool_category_edit_color: UserRank,
    pub pool_category_list: UserRank,
    pub pool_category_view: UserRank,
    pub pool_category_delete: UserRank,
    pub pool_category_set_default: UserRank,

    pub comment_create: UserRank,
    pub comment_delete_any: UserRank,
    pub comment_delete_own: UserRank,
    pub comment_edit_any: UserRank,
    pub comment_edit_own: UserRank,
    pub comment_list: UserRank,
    pub comment_view: UserRank,
    pub comment_score: UserRank,

    pub snapshot_list: UserRank,

    pub upload_create: UserRank,
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

static CONFIG: LazyLock<Config> =
    LazyLock::new(|| toml::from_str(&std::fs::read_to_string(get_config_path()).unwrap()).unwrap());

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
