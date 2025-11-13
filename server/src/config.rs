use crate::model::enums::UserRank;
use crate::string::SmallString;
use config::builder::DefaultState;
use config::{ConfigBuilder, File, FileFormat};
use lettre::message::Mailbox;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use strum::Display;
use url::Url;

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
pub struct ThumbnailConfig {
    pub avatar_width: u32,
    pub avatar_height: u32,
    pub post_width: u32,
    pub post_height: u32,
}

impl ThumbnailConfig {
    pub fn avatar_dimensions(&self) -> (u32, u32) {
        (self.avatar_width, self.avatar_height)
    }

    pub fn post_dimensions(&self) -> (u32, u32) {
        (self.post_width, self.post_height)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SmtpConfig {
    pub host: SmallString,
    pub port: Option<u16>,
    pub username: Option<SmallString>,
    pub password: Option<SmallString>,
    pub from: Mailbox,
}

#[derive(Serialize, Deserialize)]
pub struct PrivilegeConfig {
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
    pub post_edit_description: UserRank,
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
pub struct PublicConfig {
    pub name: SmallString,
    pub default_user_rank: UserRank,
    pub enable_safety: bool,
    pub contact_email: Option<SmallString>,
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
    pub privileges: PrivilegeConfig,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    data_dir: SmallString,
    pub data_url: SmallString,
    pub webhooks: Vec<Url>,
    pub password_secret: SmallString,
    pub content_secret: SmallString,
    pub domain: Option<SmallString>,
    pub delete_source_files: bool,
    pub post_similarity_threshold: f64,
    #[serde(with = "serde_regex")]
    pub pool_name_regex: Regex,
    #[serde(with = "serde_regex")]
    pub pool_category_regex: Regex,
    pub log_filter: String,
    pub auto_explain: bool,
    pub thumbnails: ThumbnailConfig,
    pub smtp: Option<SmtpConfig>,
    pub public_info: PublicConfig,
}

/// Returns a reference to the global [`Config`] object, which is deserialized
/// from the `config.toml`. Fields that are not present in `config.toml` will
/// be replaced by its default, which is specified in `config.toml.dist`.
pub fn get() -> &'static Config {
    &CONFIG
}

pub fn smtp() -> Option<&'static SmtpConfig> {
    CONFIG.smtp.as_ref()
}

pub fn privileges() -> &'static PrivilegeConfig {
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
    if DOCKER_DEPLOYMENT {
        &CONFIG.data_dir
    } else {
        static DATA_DIR: LazyLock<String> = LazyLock::new(|| {
            dotenvy::from_filename("../.env").expect(".env must be in project root directory");
            std::env::var("MOUNT_DATA").expect("MOUNT_DATA must be defined in .env")
        });
        &DATA_DIR
    }
}

pub fn port() -> u16 {
    const DEFAULT_PORT: u16 = 6666;
    std::env::var("SERVER_PORT")
        .ok()
        .and_then(|var| var.parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

pub fn database_url() -> &'static str {
    static DATABASE_URL: LazyLock<String> = LazyLock::new(|| create_url(None));
    &DATABASE_URL
}

/// Returns a url for the database using `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_HOST`, and `POSTGRES_DATABASE`
/// environment variables. If `database_override` is not `None`, then it's value will be used in place of `POSTGRES_DATABASE`.
pub fn create_url(database_override: Option<&str>) -> String {
    if !DOCKER_DEPLOYMENT {
        dotenvy::from_filename("../.env").expect(".env must be in project root directory");
    }

    let user = std::env::var("POSTGRES_USER").expect("POSTGRES_USER must be defined in .env");
    let password = std::env::var("POSTGRES_PASSWORD").expect("POSTGRES_PASSWORD must be defined in .env");
    let hostname = std::env::var("POSTGRES_HOST").unwrap_or_else(|_| String::from("localhost"));
    let database = std::env::var("POSTGRES_DB").expect("POSTGRES_DB must be defined in .env");
    let database = database_override.unwrap_or(&database);

    format!("postgres://{user}:{password}@{hostname}/{database}")
}

const DOCKER_DEPLOYMENT: bool = option_env!("DOCKER_DEPLOYMENT").is_some();
const DEFAULT_CONFIG: &str = include_str!("../config.toml.dist");

static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    // TODO: Remove in favor of config injection
    if cfg!(test) {
        return ConfigBuilder::<DefaultState>::default()
            .add_source(File::from_str(DEFAULT_CONFIG, FileFormat::Toml))
            .build()
            .and_then(config::Config::try_deserialize)
            .unwrap();
    }

    let mut config: Config = match ConfigBuilder::<DefaultState>::default()
        .add_source(File::from_str(DEFAULT_CONFIG, FileFormat::Toml))
        .add_source(File::with_name("config"))
        .build()
        .and_then(config::Config::try_deserialize)
    {
        Ok(parsed) => parsed,
        Err(err) => {
            // We use `eprintln!` instead of `error!` here because tracing hasn't been initialized yet
            eprintln!("Could not parse config.toml. Details:\n{err}");
            std::process::exit(1);
        }
    };
    config.public_info.can_send_mails = config.smtp.is_some();
    config
});
