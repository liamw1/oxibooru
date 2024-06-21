use crate::api;
use crate::config::CONFIG;
use crate::model::post::Post;
use serde::Serialize;
use time::serde::rfc3339;
use time::OffsetDateTime;
use toml::Table;
use warp::reject::Rejection;

pub async fn get_info() -> Result<api::Reply, Rejection> {
    Ok(read_info().into())
}

// TODO: Remove renames by changing references to these names in client
#[derive(Serialize)]
struct Info {
    #[serde(rename(serialize = "postCount"))]
    post_count: i64,
    #[serde(rename(serialize = "diskUsage"))]
    disk_usage: i64,
    #[serde(with = "rfc3339", rename(serialize = "serverTime"))]
    server_time: OffsetDateTime,
    config: Table,
}

fn read_required_table(name: &str) -> &'static Table {
    CONFIG
        .get(name)
        .and_then(|parsed| parsed.as_table())
        .unwrap_or_else(|| panic!("Table {name} not found in config.toml"))
}

fn read_info() -> Result<Info, api::Error> {
    let mut conn = crate::establish_connection()?;

    let info = Info {
        post_count: Post::count(&mut conn)?,
        disk_usage: 0, // TODO
        server_time: OffsetDateTime::now_utc(),
        config: read_required_table("public_info").clone(),
    };

    Ok(info)
}
