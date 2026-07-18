use crate::filesystem::Directory;
use crate::model::enums::UserRank;
use crate::search::preferences::Preferences;
use crate::string::{SecretString, SmallString};
use config::builder::DefaultState;
use config::{ConfigBuilder, File, FileFormat};
use lettre::message::Mailbox;
use percent_encoding::{AsciiSet, CONTROLS, NON_ALPHANUMERIC};
use regex::Regex;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::error::Error;
#[cfg(feature = "load_env")]
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use strum::{Display, EnumCount, EnumIter, EnumTable, IntoEnumIterator, IntoStaticStr};
use url::Url;
use utoipa::openapi::{ObjectBuilder, RefOr, Schema};
use utoipa::{PartialSchema, ToSchema};

#[derive(Debug, Default)]
pub struct Args {
    pub admin_mode: bool,
    pub env_path: Option<PathBuf>,
    pub config_path: Option<String>,
    pub ffmpeg_path: Option<PathBuf>,
}

pub struct Env {
    pub http_origin: Option<String>,
    pub http_referer: Option<String>,
    pub domain_port: Option<u16>,
    pub server_port: u16,
    postgres_user: SecretString,
    postgres_password: SecretString,
    postgres_hostname: SecretString,
    postgres_port: u16,
    postgres_database: SecretString,
}

#[derive(Debug, Display, Clone, Copy, PartialEq, Eq)]
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
    pub host: SecretString,
    pub port: Option<u16>,
    pub username: Option<SecretString>,
    pub password: Option<SecretString>,
    pub from: Mailbox,
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
            static ACTION_NAMES: LazyLock<Vec<&str>> = LazyLock::new(|| Action::iter().map(<&str>::from).collect());
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
    #[serde(skip)]
    pub args: Args,
    pub data_dir: PathBuf,
    pub data_url: String,
    pub webhooks: Vec<Url>,
    pub password_secret: SecretString,
    pub content_secret: SecretString,
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
    #[serde(default)]
    pub anonymous_preferences: Preferences,
    #[serde(default)]
    pub restricted_preferences: Preferences,
    #[serde(default)]
    pub regular_preferences: Preferences,
    #[serde(default)]
    pub power_preferences: Preferences,
    #[serde(default)]
    pub moderator_preferences: Preferences,
    #[serde(skip)] // Administrators have no server-wide preferences/blacklists
    pub administrator_preferences: Preferences,
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
        // Encode characters that could allow for file traversal, and the encode again for the URL
        let lowercase_username = username.to_lowercase();
        let encoded_username = percent_encoding::utf8_percent_encode(&lowercase_username, TRAVERSAL).to_string();
        let double_encoded_username = percent_encoding::utf8_percent_encode(&encoded_username, NON_ALPHANUMERIC);
        format!("{}/avatars/{double_encoded_username}.png", self.data_url.trim_end_matches('/'))
    }

    /// Returns path to custom user avatar on disk.
    pub fn custom_avatar_path(&self, username: &str) -> PathBuf {
        // Encode characters that could allow for file traversal
        let lowercase_username = username.to_lowercase();
        let encoded_username = percent_encoding::utf8_percent_encode(&lowercase_username, TRAVERSAL);
        let filename = format!("{encoded_username}.png");
        self.path(Directory::Avatars).join(filename)
    }
}

/// Reads commandline args.
pub fn read_args() -> Args {
    let admin_mode = std::env::args().any(|arg| arg == "--admin");
    let env_path = std::env::args().find_map(|arg| arg.strip_prefix("--env-path=").map(PathBuf::from));
    let config_path = std::env::args().find_map(|arg| arg.strip_prefix("--config-path=").map(String::from));
    let ffmpeg_path = std::env::args().find_map(|arg| arg.strip_prefix("--ffmpeg-path=").map(PathBuf::from));
    Args {
        admin_mode,
        env_path,
        config_path,
        ffmpeg_path,
    }
}

pub fn read_env(config: &Config) -> Result<Arc<Env>, Box<dyn Error>> {
    const DEFAULT_SERVER_PORT: u16 = 6666;
    const DEFAULT_POSTGRES_PORT: u16 = 5432;

    #[cfg(feature = "load_env")]
    load_dotenv(config.args.env_path.as_deref())?;

    let http_origin = std::env::var("HTTP_ORIGIN").ok();
    let http_referer = std::env::var("HTTP_REFERER").ok();
    let domain_port = std::env::var("PORT").ok().and_then(|port| port.parse().ok());

    let server_port = std::env::var("SERVER_PORT")
        .ok()
        .and_then(|var| var.parse().ok())
        .unwrap_or(DEFAULT_SERVER_PORT);

    let postgres_user = std::env::var("POSTGRES_USER").map(SecretString::from)?;
    let postgres_password = std::env::var("POSTGRES_PASSWORD").map(SecretString::from)?;
    let postgres_hostname = SecretString::from(std::env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".into()));
    let postgres_port = std::env::var("POSTGRES_PORT")
        .ok()
        .and_then(|port| port.parse().ok())
        .unwrap_or(DEFAULT_POSTGRES_PORT);
    let postgres_database = std::env::var("POSTGRES_DB").map(SecretString::from)?;

    Ok(Arc::new(Env {
        http_origin,
        http_referer,
        domain_port,
        server_port,
        postgres_user,
        postgres_password,
        postgres_hostname,
        postgres_port,
        postgres_database,
    }))
}

/// Deserializes the `config.toml`.
/// Any values not present will default to the corresponding value in `config.toml.dist`.
pub fn create(args: Args) -> Arc<Config> {
    if cfg!(test) {
        panic!("Production config disallowed in test build!")
    } else {
        Arc::new(create_config(args))
    }
}

/// Creates a test config with an optional `override_relative_path` to override the default config.
#[cfg(test)]
pub fn test_config(override_relative_path: Option<&str>) -> Config {
    let mut args = read_args();
    args.config_path = override_relative_path.map(|relative_path| format!("test/request/{relative_path}/config"));
    create_config(args)
}

/// Returns a url for the database using `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_HOST`, and `POSTGRES_DB`
/// environment variables. If `database_override` is not `None`, then it's value will be used in place of `POSTGRES_DB`.
pub fn database_url(env: &Env, database_override: Option<&str>) -> SecretString {
    // Percent-encode credentials to allow for special characters
    let port = env.postgres_port;
    let user = percent_encoding::utf8_percent_encode(env.postgres_user.read(), NON_ALPHANUMERIC);
    let password = percent_encoding::utf8_percent_encode(env.postgres_password.read(), NON_ALPHANUMERIC);
    let hostname = percent_encoding::utf8_percent_encode(env.postgres_hostname.read(), NON_ALPHANUMERIC);
    let database = percent_encoding::utf8_percent_encode(
        database_override.unwrap_or(env.postgres_database.read()),
        NON_ALPHANUMERIC,
    );
    SecretString::from(format!("postgres://{user}:{password}@{hostname}:{port}/{database}"))
}

/// Set of characters that allow for file traversal.
const TRAVERSAL: &AsciiSet = &CONTROLS.add(b'/').add(b'\\').add(b'.').add(b':').add(b'%');

const DEFAULT_CONFIG: &str = include_str!("../config.toml.dist");

#[cfg(feature = "load_env")]
fn load_dotenv(env_path: Option<&Path>) -> dotenvy::Result<PathBuf> {
    if let Some(env_path) = env_path {
        // If env_path is specified in args, read from that path
        dotenvy::from_filename(env_path)
    } else {
        // Otherwise, try to read a `.env` from the working directory or its parent
        dotenvy::from_filename(".env").or_else(|_| dotenvy::from_filename("../.env"))
    }
}

fn create_config(args: Args) -> Config {
    let mut config_builder =
        ConfigBuilder::<DefaultState>::default().add_source(File::from_str(DEFAULT_CONFIG, FileFormat::Toml));
    if let Some(path) = args.config_path.as_deref() {
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
    config.args = args;

    // Accumulate preferences from higher user ranks
    config.power_preferences.merge(&config.moderator_preferences);
    config.regular_preferences.merge(&config.power_preferences);
    config.restricted_preferences.merge(&config.regular_preferences);
    config.anonymous_preferences.merge(&config.restricted_preferences);

    config
}
