use crate::api::ApiError;
use crate::config::CONFIG;
use crate::model::post::Post;
use chrono::Utc;
use serde::Serialize;
use toml::Table;
use warp::reject::Rejection;
use warp::reply::Reply;

pub async fn get_info() -> Result<Box<dyn Reply>, Rejection> {
    Ok(match collect_info() {
        Ok(info) => Box::new(warp::reply::json(&info)),
        Err(err) => Box::new(err.to_reply()),
    })
}

// TODO: Remove renames by changing references to these names in client
#[derive(Serialize)]
struct Info {
    #[serde(rename(serialize = "postCount"))]
    post_count: i64,
    #[serde(rename(serialize = "diskUsage"))]
    disk_usage: i64,
    #[serde(rename(serialize = "serverTime"))]
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
