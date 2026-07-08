use crate::filesystem::Directory;
use crate::model::enums::UserRank;
use crate::string::SmallString;
use config::builder::DefaultState;
use config::{ConfigBuilder, File, FileFormat};
use lettre::message::Mailbox;
use regex::Regex;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::ToOwned;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use strum::{Display, EnumCount, EnumIter, EnumTable, IntoEnumIterator, IntoStaticStr};
use url::Url;
use utoipa::openapi::{ObjectBuilder, RefOr, Schema};
use utoipa::{PartialSchema, ToSchema};

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

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
pub struct SmtpConfig {
    pub host: SmallString,
    pub port: Option<u16>,
    pub username: Option<SmallString>,
    pub password: Option<SmallString>,
    pub from: Mailbox,
}

#[derive(Debug, Deserialize)]
pub struct AnonymousPreferences {
    pub tag_blacklist: Vec<SmallString>,
    pub tag_category_blacklist: Vec<SmallString>,
    pub hide_unsafe: bool,
    pub hide_sketchy: bool,
    pub hide_untagged: bool,
}

impl AnonymousPreferences {
    pub fn is_empty(&self) -> bool {
        self.tag_blacklist.is_empty()
            && self.tag_category_blacklist.is_empty()
            && !self.hide_unsafe
            && !self.hide_sketchy
            && !self.hide_untagged
    }
}

#[derive(Clone, Copy, EnumCount, EnumIter, EnumTable, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum Action {
    UserCreateSelf,
    UserCreateAny,
    UserList,
    UserView,
    UserEditAnyName,
    UserEditAnyPass,
    UserEditAnyEmail,
    UserEditAnyAvatar,
    UserEditAnyRank,
    UserEditSelfName,
    UserEditSelfPass,
    UserEditSelfEmail,
    UserEditSelfAvatar,
    UserEditSelfRank,
    UserDeleteAny,
    UserDeleteSelf,

    UserTokenListAny,
    UserTokenListSelf,
    UserTokenCreateAny,
    UserTokenCreateSelf,
    UserTokenEditAny,
    UserTokenEditSelf,
    UserTokenDeleteAny,
    UserTokenDeleteSelf,

    PostCreateAnonymous,
    PostCreateIdentified,
    PostList,
    PostReverseSearch,
    PostView,
    PostViewFeatured,
    PostEditContent,
    PostEditDescription,
    PostEditFlag,
    PostEditNote,
    PostEditRelation,
    PostEditSafety,
    PostEditSource,
    PostEditTag,
    PostEditThumbnail,
    PostFeature,
    PostDelete,
    PostScore,
    PostMerge,
    PostFavorite,
    PostBulkEditTag,
    PostBulkEditSafety,
    PostBulkEditDelete,

    TagCreate,
    TagEditName,
    TagEditCategory,
    TagEditDescription,
    TagEditImplication,
    TagEditSuggestion,
    TagList,
    TagView,
    TagMerge,
    TagDelete,

    TagCategoryCreate,
    TagCategoryEditName,
    TagCategoryEditColor,
    TagCategoryEditOrder,
    TagCategoryList,
    TagCategoryView,
    TagCategoryDelete,
    TagCategorySetDefault,

    PoolCreate,
    PoolEditName,
    PoolEditCategory,
    PoolEditDescription,
    PoolEditPost,
    PoolList,
    PoolView,
    PoolMerge,
    PoolDelete,

    PoolCategoryCreate,
    PoolCategoryEditName,
    PoolCategoryEditColor,
    PoolCategoryList,
    PoolCategoryView,
    PoolCategoryDelete,
    PoolCategorySetDefault,

    CommentCreate,
    CommentDeleteAny,
    CommentDeleteOwn,
    CommentEditAny,
    CommentEditOwn,
    CommentList,
    CommentView,
    CommentScore,

    SnapshotList,

    UploadCreate,
    UploadUseDownloader,
}

pub type PrivilegeConfig = ActionTable<UserRank>;

impl Serialize for PrivilegeConfig {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("PrivilegeConfig", Action::COUNT)?;
        for action in Action::iter() {
            state.serialize_field(action.into(), &self[action])?;
        }
        state.end()
    }
}

impl<'de> Deserialize<'de> for PrivilegeConfig {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut required_ranks = PrivilegeConfig::filled(UserRank::Administrator);
        let mut privilege_map = HashMap::<String, UserRank>::deserialize(deserializer)?;

        for action in Action::iter() {
            let action_name: &'static str = action.into();
            required_ranks[action] = privilege_map
                .remove(action_name)
                .ok_or(serde::de::Error::missing_field(action_name))?;
        }
        if let Some(unknown_field) = privilege_map.keys().next() {
            static ACTION_NAMES: LazyLock<Vec<&str>> =
                LazyLock::new(|| Action::iter().map(<&'static str>::from).collect());
            return Err(serde::de::Error::unknown_field(unknown_field, &ACTION_NAMES));
        }
        Ok(required_ranks)
    }
}

impl PartialSchema for PrivilegeConfig {
    fn schema() -> RefOr<Schema> {
        let mut builder = ObjectBuilder::new();
        for action in Action::iter() {
            let name: &'static str = action.into();
            builder = builder.property(name, UserRank::schema()).required(name);
        }
        RefOr::T(Schema::Object(builder.build()))
    }
}

impl ToSchema for PrivilegeConfig {}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(rename_all = "camelCase")] // ToSchema doesn't detect serde(rename_all(serialize = ...))
#[serde(deny_unknown_fields, rename_all(serialize = "camelCase"))]
pub struct PublicConfig {
    pub name: SmallString,
    pub default_user_rank: UserRank,
    pub enable_safety: bool,
    pub contact_email: Option<SmallString>,
    #[serde(skip)]
    pub can_send_mails: bool,
    #[schema(rename = "userNameRegex", value_type = String, format = Regex)]
    #[serde(rename(serialize = "userNameRegex"), with = "serde_regex")]
    pub username_regex: Regex,
    #[schema(value_type = String, format = Regex)]
    #[serde(with = "serde_regex")]
    pub password_regex: Regex,
    #[schema(value_type = String, format = Regex)]
    #[serde(with = "serde_regex")]
    pub tag_name_regex: Regex,
    #[schema(value_type = String, format = Regex)]
    #[serde(with = "serde_regex")]
    pub tag_category_name_regex: Regex,
    pub privileges: PrivilegeConfig,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub data_dir: PathBuf,
    pub data_url: String,
    pub webhooks: Vec<Url>,
    pub password_secret: SmallString,
    pub content_secret: SmallString,
    pub domain: Option<SmallString>,
    pub delete_source_files: bool,
    pub append_tag_implications_on_post_edit: bool,
    pub post_similarity_threshold: f64,
    #[serde(with = "serde_regex")]
    pub pool_name_regex: Regex,
    #[serde(with = "serde_regex")]
    pub pool_category_regex: Regex,
    pub log_filter: String,
    pub auto_explain: bool,
    pub thumbnails: ThumbnailConfig,
    pub smtp: Option<SmtpConfig>,
    pub anonymous_preferences: AnonymousPreferences,
    pub public_info: PublicConfig,
}

impl Config {
    pub fn smtp(&self) -> Option<&SmtpConfig> {
        self.smtp.as_ref()
    }

    pub fn default_rank(&self) -> UserRank {
        // Default user rank can't be anonymous
        std::cmp::max(self.public_info.default_user_rank, UserRank::Restricted)
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
pub fn create() -> Arc<Config> {
    if cfg!(test) {
        panic!("Production config disallowed in test build!")
    } else {
        let config_path = std::env::args().find_map(|arg| arg.strip_prefix("--config-path=").map(ToOwned::to_owned));
        let config_path = config_path.as_deref().unwrap_or("config");
        Arc::new(create_config(Some(config_path)))
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

/// Returns a url for the database using `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_HOST`, and `POSTGRES_DB`
/// environment variables. If `database_override` is not `None`, then it's value will be used in place of `POSTGRES_DB`.
pub fn database_url(database_override: Option<&str>) -> String {
    const DEFAULT_POSTGRES_PORT: u16 = 5432;
    let user = std::env::var("POSTGRES_USER").expect("POSTGRES_USER must be defined in .env");
    let password = std::env::var("POSTGRES_PASSWORD").expect("POSTGRES_PASSWORD must be defined in .env");
    let hostname = std::env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".into());
    let port = std::env::var("POSTGRES_PORT")
        .ok()
        .and_then(|port| port.parse().ok())
        .unwrap_or(DEFAULT_POSTGRES_PORT);
    let database = std::env::var("POSTGRES_DB").expect("POSTGRES_DB must be defined in .env");
    let database = database_override.unwrap_or(&database);

    format!("postgres://{user}:{password}@{hostname}:{port}/{database}")
}

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
