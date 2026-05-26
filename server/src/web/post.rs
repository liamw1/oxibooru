use crate::app::{AppState, Context};
use crate::config::Action;
use crate::extract::{Ctx, Json, PageParams, Path, Query, ResourceParams};
use crate::model::enums::{PostFlag, PostSafety, PostType};
use crate::resource::NotRequested;
use crate::resource::post::{Field, PostInfo};
use crate::web::pager::{Page, Pager};
use crate::web::{SearchQuery, Tab};
use crate::{api, time, unit};
use askama::Template;
use axum::response::Html;
use axum::{Router, routing};
use strum::IntoEnumIterator;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/posts", routing::get(gallery))
        .route("/post/{post_id}", routing::get(main))
}

#[derive(PartialEq, Eq)]
pub enum EditMode {
    Tag,
    Safety,
    Delete,
}

#[derive(Template)]
#[template(path = "pages/post_gallery.html")]
struct GalleryTemplate {
    ctx: Context,
    active_tab: Tab,
    edit_mode: Option<EditMode>,
    posts: Vec<PostInfo>,
    pager: Pager,
    query: Option<String>,
}

async fn gallery(
    ctx: Ctx,
    Query(SearchQuery { query }): Query<SearchQuery>,
    page_params: Query<PageParams>,
) -> Html<String> {
    let fields = [
        Field::Id,
        Field::Tags,
        Field::ThumbnailUrl,
        Field::Type,
        Field::Safety,
        Field::Score,
        Field::FavoriteCount,
        Field::CommentCount,
    ]
    .into();
    let resource_params = Query(ResourceParams { query, fields });
    let Json(response) = api::post::list(ctx.clone(), resource_params, page_params)
        .await
        .unwrap();

    let pager = Pager::build("posts", page_params, &response);

    let Ctx(ctx, _) = ctx;
    GalleryTemplate {
        ctx,
        active_tab: Tab::Post,
        edit_mode: None,
        posts: response.results,
        pager,
        query: response.query,
    }
    .render()
    .map(Html)
    .unwrap()
}

#[derive(Template)]
#[template(path = "pages/post_main.html")]
struct PostMain {
    ctx: Context,
    active_tab: Tab,
    is_editing: bool,
    post: PostInfo,
    prev_post: Option<PostInfo>,
    next_post: Option<PostInfo>,
    query: Option<String>,
}

impl PostMain {
    fn full_content_url(&self) -> Result<String, NotRequested> {
        self.post.content_url().map(|url| self.ctx.full_url(url))
    }
}

async fn main(ctx: Ctx, post_id: Path<i64>, Query(SearchQuery { query }): Query<SearchQuery>) -> Html<String> {
    let fields = [
        Field::Id,
        Field::User,
        Field::FileSize,
        Field::CanvasWidth,
        Field::CanvasHeight,
        Field::Safety,
        Field::Type,
        Field::MimeType,
        Field::ChecksumMd5,
        Field::Flags,
        Field::Source,
        Field::Description,
        Field::CreationTime,
        Field::ContentUrl,
        Field::ThumbnailUrl,
        Field::Tags,
        Field::Relations,
    ]
    .into();
    let resource_params = Query(ResourceParams {
        query: query.clone(),
        fields,
    });
    let Json(post) = api::post::get(ctx.clone(), post_id, resource_params.clone())
        .await
        .unwrap();
    let Json(neighbors) = api::post::get_neighbors(ctx.clone(), post_id, resource_params)
        .await
        .unwrap();

    let Ctx(ctx, _) = ctx;
    PostMain {
        ctx,
        active_tab: Tab::Post,
        is_editing: false,
        post,
        prev_post: neighbors.prev,
        next_post: neighbors.next,
        query,
    }
    .render()
    .map(Html)
    .unwrap()
}
