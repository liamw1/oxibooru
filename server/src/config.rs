use crate::filesystem::Directory;
use crate::model::enums::UserRank;
use crate::string::SmallString;
use config::builder::DefaultState;
use config::{ConfigBuilder, File, FileFormat};
use lettre::message::Mailbox;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
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

#[derive(Deserialize)]
pub struct SmtpConfig {
    pub host: SmallString,
    pub port: Option<u16>,
    pub username: Option<SmallString>,
    pub password: Option<SmallString>,
    pub from: Mailbox,
}

#[derive(Clone, Serialize, Deserialize)]
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

#[derive(Clone, Serialize, Deserialize)]
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
    pub data_dir: PathBuf,
    pub data_url: String,
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

impl Config {
    pub fn smtp(&self) -> Option<&SmtpConfig> {
        self.smtp.as_ref()
    }

    pub fn default_rank(&self) -> UserRank {
        self.public_info.default_user_rank
    }

    pub fn privileges(&self) -> &PrivilegeConfig {
        &self.public_info.privileges
    }

    pub fn regex(&self, regex_type: RegexType) -> &Regex {
        match regex_type {
            RegexType::Pool => &self.pool_name_regex,
            RegexType::PoolCategory => &self.pool_category_regex,
            RegexType::Tag => &self.public_info.tag_name_regex,
            RegexType::TagCategory => &self.public_info.tag_category_name_regex,
            RegexType::Username => &self.public_info.username_regex,
            RegexType::Password => &self.public_info.password_regex,
        }
    }

    pub fn path(&self, directory: Directory) -> PathBuf {
        let folder: &str = directory.into();
        self.data_dir.join(folder)
    }

    /// Returns URL to custom user avatar.
    pub fn custom_avatar_url(&self, username: &str) -> String {
        format!("{}/avatars/{}.png", self.data_url, username.to_lowercase())
    }

    /// Returns path to custom user avatar on disk.
    pub fn custom_avatar_path(&self, username: &str) -> PathBuf {
        let filename = format!("{}.png", username.to_lowercase());
        self.path(Directory::Avatars).join(filename)
    }
}

/// Deserializes the `config.toml`.
/// Any values not present will default to the corresponding value in `config.toml.dist`.
pub fn create() -> Config {
    if cfg!(test) {
        panic!("Production config disallowed in test build!")
    } else {
        create_config(Some("config"))
    }
}

/// Creates a test config with an optional `override_relative_path` to override the default config.
#[cfg(test)]
pub fn test_config(override_relative_path: Option<&str>) -> Config {
    let override_path = override_relative_path.map(|relative_path| format!("test/request/{relative_path}/config"));
    create_config(override_path.as_deref())
}

pub fn port() -> u16 {
    const DEFAULT_PORT: u16 = 6666;
    std::env::var("SERVER_PORT")
        .ok()
        .and_then(|var| var.parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

/// Returns a url for the database using `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_HOST`, and `POSTGRES_DATABASE`
/// environment variables. If `database_override` is not `None`, then it's value will be used in place of `POSTGRES_DATABASE`.
pub fn database_url(database_override: Option<&str>) -> String {
    if !DOCKER_DEPLOYMENT {
        dotenvy::from_filename("../.env").expect(".env must be in project root directory");
    }

    let user = std::env::var("POSTGRES_USER").expect("POSTGRES_USER must be defined in .env");
    let password = std::env::var("POSTGRES_PASSWORD").expect("POSTGRES_PASSWORD must be defined in .env");
    let hostname = std::env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".into());
    let database = std::env::var("POSTGRES_DB").expect("POSTGRES_DB must be defined in .env");
    let database = database_override.unwrap_or(&database);

    format!("postgres://{user}:{password}@{hostname}/{database}")
}

const DOCKER_DEPLOYMENT: bool = option_env!("DOCKER_DEPLOYMENT").is_some();
const DEFAULT_CONFIG: &str = include_str!("../config.toml.dist");

fn create_config(config_path: Option<&str>) -> Config {
    let mut config_builder =
        ConfigBuilder::<DefaultState>::default().add_source(File::from_str(DEFAULT_CONFIG, FileFormat::Toml));
    if let Some(path) = config_path {
        config_builder = config_builder.add_source(File::with_name(path));
    }

    let mut config: Config = match config_builder.build().and_then(config::Config::try_deserialize) {
        Ok(parsed) => parsed,
        Err(err) => {
            // We use `eprintln!` instead of `error!` here because tracing hasn't been initialized yet
            eprintln!("Could not parse config.toml. Details:\n{err}");
            std::process::exit(1);
        }
    };
    config.public_info.can_send_mails = config.smtp.is_some();
    config
}
