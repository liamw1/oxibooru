use crate::model::enums::UserRank;
use lettre::message::Mailbox;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::LazyLock;
use strum::Display;

#[derive(Debug, Display, Clone, Copy)]
#[strum(serialize_all = "lowercase")]
pub enum RegexType {
    Pool,
    #[strum(serialize = "pool category")]
    PoolCategory,
    Tag,
    #[strum(serialize = "tag category")]
    TagCategory,
    Username,
    Password,
}

#[derive(Deserialize)]
pub struct Thumbnails {
    pub avatar_width: u32,
    pub avatar_height: u32,
    pub post_width: u32,
    pub post_height: u32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SmtpInfo {
    pub username: String,
    pub password: String,
    pub from: Mailbox,
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
#[serde(deny_unknown_fields)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct PublicInfo {
    pub name: String,
    pub default_user_rank: UserRank,
    pub enable_safety: bool,
    pub contact_email: Option<String>,
    #[serde(skip_deserializing)]
    pub can_send_mails: bool,
    #[serde(with = "serde_regex")]
    #[serde(rename(serialize = "userNameRegex"))]
    pub username_regex: Regex,
    #[serde(with = "serde_regex")]
    pub password_regex: Regex,
    #[serde(with = "serde_regex")]
    pub tag_name_regex: Regex,
    #[serde(with = "serde_regex")]
    pub tag_category_name_regex: Regex,
    pub smtp: Option<SmtpInfo>,
    pub privileges: Privileges,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    data_dir: String,
    pub data_url: String,
    pub password_secret: String,
    pub content_secret: String,
    pub domain: Option<String>,
    pub delete_source_files: bool,
    pub post_similarity_threshold: f64,
    #[serde(with = "serde_regex")]
    pub pool_name_regex: Regex,
    #[serde(with = "serde_regex")]
    pub pool_category_regex: Regex,
    pub thumbnails: Thumbnails,
    pub public_info: PublicInfo,
}

pub fn get() -> &'static Config {
    &CONFIG
}

pub fn smtp() -> Option<&'static SmtpInfo> {
    CONFIG.public_info.smtp.as_ref()
}

pub fn privileges() -> &'static Privileges {
    &CONFIG.public_info.privileges
}

/// Gets Regexes that are used to filter valid names for pools, tags, usernames, etc.
pub fn regex(regex_type: RegexType) -> &'static Regex {
    match regex_type {
        RegexType::Pool => &CONFIG.pool_name_regex,
        RegexType::PoolCategory => &CONFIG.pool_category_regex,
        RegexType::Tag => &CONFIG.public_info.tag_name_regex,
        RegexType::TagCategory => &CONFIG.public_info.tag_category_name_regex,
        RegexType::Username => &CONFIG.public_info.username_regex,
        RegexType::Password => &CONFIG.public_info.password_regex,
    }
}

/// The rank of an anonymous user.
pub fn default_rank() -> UserRank {
    CONFIG.public_info.default_user_rank
}

pub fn data_dir() -> &'static str {
    &DATA_DIR
}

pub fn database_url() -> &'static str {
    &DATABASE_URL
}

pub fn port() -> u16 {
    const DEFAULT_PORT: u16 = 6666;
    std::env::var("PORT")
        .ok()
        .and_then(|var| var.parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    let mut config: Config = toml::from_str(&std::fs::read_to_string(get_config_path()).unwrap()).unwrap();
    config.public_info.can_send_mails = config.public_info.smtp.is_some();
    config
});

static DATA_DIR: LazyLock<Cow<str>> = LazyLock::new(|| match std::env::var("DOCKER_DEPLOYMENT") {
    Ok(_) => Cow::Borrowed(&CONFIG.data_dir),
    Err(_) => {
        dotenvy::from_filename("../.env").unwrap();
        Cow::Owned(std::env::var("MOUNT_DATA").unwrap())
    }
});

static DATABASE_URL: LazyLock<String> = LazyLock::new(|| {
    if std::env::var("DOCKER_DEPLOYMENT").is_err() {
        dotenvy::from_filename("../.env").unwrap();
    }

    let user = std::env::var("POSTGRES_USER").unwrap();
    let password = std::env::var("POSTGRES_PASSWORD").unwrap();
    let database = std::env::var("POSTGRES_DB").unwrap();
    let hostname = match std::env::var("DOCKER_DEPLOYMENT") {
        Ok(_) => "host.docker.internal",
        Err(_) => "localhost",
    };

    format!("postgres://{user}:{password}@{hostname}/{database}")
});

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
