use crate::api::ApiError;
use crate::config::CONFIG;
use crate::model::post::Post;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use toml::Table;
use warp::reject::Rejection;
use warp::reply::Reply;

pub async fn get_info() -> Result<Box<dyn Reply>, Rejection> {
    match collect_info() {
        Ok(info) => Ok(Box::new(warp::reply::json(&info))),
        Err(err) => err.to_reply().map(|reply| Box::new(reply) as Box<dyn Reply>),
    }
}

#[derive(Deserialize, Serialize, Clone)]
struct Config {
    name: String,
    username_regex: String,
    password_regex: String,
    tag_name_regex: String,
    tag_category_regex: String,
    default_user_rank: String,
    enable_safety: bool,
    contact_email: String,
    can_send_mails: bool,
    privileges: Table,
}

#[derive(Deserialize, Serialize, Clone)]
struct Info {
    post_count: i64,
    disk_usage: i64,
    server_time: String,
    config: Config,
}

fn read_required_config(name: &str) -> &'static str {
    CONFIG
        .get(name)
        .and_then(|parsed| parsed.as_str())
        .unwrap_or_else(|| panic!("Config {name} not found in config.toml"))
}

fn read_required_boolean_config(name: &str) -> bool {
    CONFIG
        .get(name)
        .and_then(|parsed| parsed.as_bool())
        .unwrap_or_else(|| panic!("Boolean config {name} not found in config.toml"))
}

fn read_optional_config(name: &str, default: &'static str) -> &'static str {
    CONFIG.get(name).and_then(|parsed| parsed.as_str()).unwrap_or(default)
}

fn read_required_table(name: &str) -> &'static Table {
    CONFIG
        .get(name)
        .and_then(|parsed| parsed.as_table())
        .unwrap_or_else(|| panic!("Table {name} not found in config.toml"))
}

fn collect_info() -> Result<Info, ApiError> {
    let config = Config {
        name: read_required_config("name").to_owned(),
        username_regex: read_required_config("username_regex").to_owned(),
        password_regex: read_required_config("password_regex").to_owned(),
        tag_name_regex: read_required_config("tag_name_regex").to_owned(),
        tag_category_regex: read_required_config("tag_category_regex").to_owned(),
        default_user_rank: read_required_config("default_user_rank").to_owned(),
        enable_safety: read_required_boolean_config("enable_safety"),
        contact_email: read_optional_config("contact_email", "").to_owned(),
        can_send_mails: false, // TODO
        privileges: read_required_table("privileges").clone(),
    };

    let mut conn = crate::establish_connection()?;
    let info = Info {
        post_count: Post::count(&mut conn)?,
        disk_usage: 0, // TODO
        server_time: Utc::now().to_string(),
        config,
    };

    Ok(info)
}
