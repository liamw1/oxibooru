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
struct Info {
    post_count: i64,
    disk_usage: i64,
    server_time: String,
    config: Table,
}

fn read_required_table(name: &str) -> &'static Table {
    CONFIG
        .get(name)
        .and_then(|parsed| parsed.as_table())
        .unwrap_or_else(|| panic!("Table {name} not found in config.toml"))
}

fn collect_info() -> Result<Info, ApiError> {
    let mut conn = crate::establish_connection()?;
    let info = Info {
        post_count: Post::count(&mut conn)?,
        disk_usage: 0, // TODO
        server_time: Utc::now().to_string(),
        config: read_required_table("public_info").clone(),
    };

    Ok(info)
}
