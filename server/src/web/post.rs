use crate::app::{AppState, Context};
use crate::config::Action;
use crate::extract::{Ctx, Json, PageParams, Path, Query, ResourceParams};
use crate::model::enums::{PostFlag, PostSafety, PostType};
use crate::resource::NotRequested;
use crate::resource::post::{Field, PostInfo};
use crate::web::Tab;
use crate::web::pager::{Page, Pager};
use crate::{api, time, unit};
use askama::Template;
use axum::response::Html;
use axum::{Router, routing};
use serde::{Deserialize, Serialize};
use strum::{Display, IntoEnumIterator};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/posts", routing::get(gallery))
        .route("/post/{post_id}", routing::get(main))
}

#[derive(Clone, Copy, Display, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum EditMode {
    Tag,
    Safety,
    Delete,
}

const SAFE_DEFAULT: bool = true;
const SKETCHY_DEFAULT: bool = true;
const UNSAFE_DEFAULT: bool = false;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Params {
    search_text: Option<String>,
    safe: Option<bool>,
    sketchy: Option<bool>,
    #[serde(rename = "unsafe")]
    unsafe_: Option<bool>,
    edit: Option<EditMode>,
}

impl Params {
    fn safe_enabled(&self) -> bool {
        self.safe.unwrap_or(SAFE_DEFAULT)
    }

    fn sketchy_enabled(&self) -> bool {
        self.sketchy.unwrap_or(SKETCHY_DEFAULT)
    }

    fn unsafe_enabled(&self) -> bool {
        self.unsafe_.unwrap_or(UNSAFE_DEFAULT)
    }

    fn search_text(&self) -> &str {
        self.search_text.as_deref().unwrap_or("")
    }

    fn query(&self) -> Option<String> {
        let safety_filter = match (self.safe_enabled(), self.sketchy_enabled(), self.unsafe_enabled()) {
            (false, false, false) => Some("-safety:safe,sketchy,unsafe"),
            (false, false, true) => Some("safety:unsafe"),
            (false, true, false) => Some("safety:sketchy"),
            (false, true, true) => Some("-safety:safe"),
            (true, false, false) => Some("safety:safe"),
            (true, false, true) => Some("-safety:sketchy"),
            (true, true, false) => Some("-safety:unsafe"),
            (true, true, true) => None,
        };
        match (self.search_text.clone(), safety_filter) {
            (None, safety_filter) => safety_filter.map(str::to_string),
            (search_text, None) => search_text,
            (Some(search_text), Some(safety_filter)) => Some(format!("{search_text} {safety_filter}")),
        }
    }

    fn simplify(mut self) -> Self {
        if self.safe == Some(SAFE_DEFAULT) {
            self.safe = None;
        }
        if self.sketchy == Some(SKETCHY_DEFAULT) {
            self.sketchy = None;
        }
        if self.unsafe_ == Some(UNSAFE_DEFAULT) {
            self.unsafe_ = None;
        }
        self
    }
}

#[derive(Template)]
#[template(path = "pages/post_gallery.html")]
struct GalleryTemplate<'a> {
    ctx: Context,
    active_tab: Tab,
    posts: Vec<PostInfo>,
    pager: Pager<'a, Params>,
    params: &'a Params,
}

async fn gallery(ctx: Ctx, Query(params): Query<Params>, page_params: Query<PageParams>) -> Html<String> {
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

    let query = params.query();
    let resource_params = Query(ResourceParams { query, fields });
    let Json(response) = api::post::list(ctx.clone(), resource_params, page_params)
        .await
        .unwrap();

    let params = params.simplify();
    let pager = Pager::build("posts", &params, page_params, response.total);

    let Ctx(ctx, _) = ctx;
    GalleryTemplate {
        ctx,
        active_tab: Tab::Post,
        posts: response.results,
        pager,
        params: &params,
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
    params: Params,
}

impl PostMain {
    fn full_content_url(&self) -> Result<String, NotRequested> {
        self.post.content_url().map(|url| self.ctx.full_url(url))
    }
}

async fn main(ctx: Ctx, post_id: Path<i64>, Query(params): Query<Params>) -> Html<String> {
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

    let query = params.query();
    let resource_params = Query(ResourceParams { query, fields });
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
        params,
    }
    .render()
    .map(Html)
    .unwrap()
}
