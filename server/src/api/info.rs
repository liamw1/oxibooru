use crate::api;
use crate::config::CONFIG;
use crate::model::post::Post;
use crate::util::DateTime;
use serde::Serialize;
use toml::Table;
use warp::reject::Rejection;

pub async fn get_info() -> Result<api::Reply, Rejection> {
    Ok(read_info().into())
}

// TODO: Remove renames by changing references to these names in client
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Info {
    post_count: i64,
    disk_usage: i64,
    featured_post: Option<i64>,
    featuring_time: Option<DateTime>,
    featuring_user: Option<String>,
    server_time: DateTime,
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
        featured_post: None,
        featuring_time: None,
        featuring_user: None,
        server_time: DateTime::now(),
        config: read_required_table("public_info").clone(),
    };

    Ok(info)
}
