use crate::api::ApiResult;
use crate::schema::post;
use crate::util::DateTime;
use crate::{api, config};
use diesel::prelude::*;
use serde::Serialize;
use std::convert::Infallible;
use std::path::Path;
use warp::{Filter, Rejection, Reply};

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let get_info = warp::get().and(warp::path!("info")).and_then(get_info_endpoint);

    get_info
}

// TODO: Remove renames by changing references to these names in client
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Info {
    post_count: i64,
    disk_usage: u64,
    featured_post: Option<i64>,
    featuring_time: Option<DateTime>,
    featuring_user: Option<String>,
    server_time: DateTime,
    config: &'static config::PublicInfo,
}

async fn get_info_endpoint() -> Result<api::Reply, Infallible> {
    Ok(get_info().into())
}

fn get_info() -> ApiResult<Info> {
    let disk_usage = calculate_directory_size(Path::new(&config::get().data_dir))?;

    let mut conn = crate::establish_connection()?;
    Ok(Info {
        post_count: post::table.count().first(&mut conn)?,
        disk_usage,
        featured_post: None,
        featuring_time: None,
        featuring_user: None,
        server_time: DateTime::now(),
        config: &config::get().public_info,
    })
}

fn calculate_directory_size(path: &Path) -> std::io::Result<u64> {
    let mut total_size = 0;
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let path = entry?.path();
            total_size += calculate_directory_size(&path)?;
        }
    } else {
        total_size += std::fs::metadata(path)?.len();
    }
    Ok(total_size)
}
