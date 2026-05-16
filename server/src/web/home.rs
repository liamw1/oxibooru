use crate::api::info;
use crate::app::{AppState, Context};
use crate::config::Action;
use crate::extract::{Ctx, Json, Query, ResourceParams};
use crate::model::enums::{MimeType, PostFlag, PostFlags, PostType};
use crate::resource::post::Field;
use crate::resource::user::MicroUser;
use crate::time::{BUILD_DATE, DateTime};
use crate::{time, unit};
use askama::Template;
use axum::response::Html;
use axum::{Router, routing};

pub fn routes() -> Router<AppState> {
    Router::new().route("/", routing::get(home))
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

struct FeaturedPostInfo {
    id: i64,
    user: Option<MicroUser>,
    canvas_width: i32,
    canvas_height: i32,
    type_: PostType,
    mime_type: MimeType,
    flags: PostFlags,
    creation_time: DateTime,
    content_url: String,
    thumbnail_url: String,
    time_since_post: String,
}

struct ServerInfo {
    post_count: i64,
    disk_usage: String,
    featured_post: Option<FeaturedPostInfo>,
    time_since_build: String,
}

#[derive(Template)]
#[template(path = "pages/home.html")]
struct HomeTemplate {
    ctx: Context,
    info: ServerInfo,
}

async fn home(ctx: Ctx) -> Html<String> {
    let fields = [
        Field::Id,
        Field::User,
        Field::CanvasWidth,
        Field::CanvasHeight,
        Field::Type,
        Field::MimeType,
        Field::Flags,
        Field::CreationTime,
        Field::ContentUrl,
        Field::ThumbnailUrl,
    ]
    .into();
    let resource_params = Query(ResourceParams { query: None, fields });
    let Json(response) = info::get(ctx.clone(), resource_params).await.unwrap();

    let featured_post = response.featured_post.map(|post| FeaturedPostInfo {
        id: post.id.unwrap(),
        user: post.user.unwrap(),
        canvas_width: post.canvas_width.unwrap(),
        canvas_height: post.canvas_height.unwrap(),
        type_: post.type_.unwrap(),
        mime_type: post.mime_type.unwrap(),
        flags: post.flags.unwrap(),
        creation_time: post.creation_time.unwrap(),
        content_url: post.content_url.unwrap(),
        thumbnail_url: post.thumbnail_url.unwrap(),
        time_since_post: time::since(post.creation_time.unwrap()),
    });
    let info = ServerInfo {
        post_count: response.post_count,
        disk_usage: unit::format_bytes(u64::try_from(response.disk_usage).unwrap_or(0)),
        featured_post,
        time_since_build: time::since(BUILD_DATE),
    };

    let Ctx(ctx, _) = ctx;
    Html(HomeTemplate { ctx, info }.render().unwrap())
}
