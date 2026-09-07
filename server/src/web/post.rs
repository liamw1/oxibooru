use crate::app::AppState;
use crate::config::Action;
use crate::extract::{Ctx, Json, Offset, Path, Query, ResourceParams};
use crate::model::enums::{PostFlag, PostSafety, PostType, Rating};
use crate::resource::NotRequested;
use crate::resource::post::{Field, IdJoinExt, Mode, PostInfo};
use crate::resource::tag_category::TagCategoryInfo;
use crate::web::pager::{Page, Pager};
use crate::web::{Html, Tab, WebError, WebResult};
use crate::{api, time, unit, web};
use askama::Template;
use axum::{Router, routing};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;
use strum::{Display, IntoEnumIterator};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/posts", routing::get(gallery))
        .route("/post/{post_id}", routing::get(view))
        .route("/post/{post_id}/edit", routing::get(edit))
}

const SAFE_DEFAULT: bool = true;
const SKETCHY_DEFAULT: bool = true;
const UNSAFE_DEFAULT: bool = false;

const LIMIT: NonZeroU64 = NonZeroU64::new(42).unwrap();

#[derive(Clone, Copy, Display, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
enum EditMode {
    Tag,
    Safety,
    Delete,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ListParams {
    safe: Option<bool>,
    sketchy: Option<bool>,
    #[serde(rename = "unsafe")]
    unsafe_: Option<bool>,
    edit: Option<EditMode>,
    search_text: Option<String>,
}

impl ListParams {
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
        if self.search_text.as_ref().is_some_and(String::is_empty) {
            self.search_text = None;
        }
        self
    }

    fn to_main_params(&self) -> MainParams {
        MainParams {
            search_text: self.search_text.clone(),
            fit: None,
        }
    }
}

#[derive(Template)]
#[template(path = "pages/post/gallery.html")]
struct GalleryTemplate<'a> {
    ctx: Ctx,
    active_tab: Tab,
    posts: Vec<PostInfo>,
    pager: Pager<'a, ListParams>,
    params: &'a ListParams,
}

async fn gallery(ctx: Ctx, Query(params): Query<ListParams>, Query(offset): Query<Offset>) -> WebResult<Html> {
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
    let page_params = Query(offset.to_page_params(LIMIT));
    let Json(response) = api::post::list(ctx.clone(), resource_params, page_params).await?;

    let params = params.simplify();
    let pager = Pager::build("posts", &params, page_params, response.total);
    GalleryTemplate {
        ctx,
        active_tab: Tab::Post,
        posts: response.results,
        pager,
        params: &params,
    }
    .render()
    .map(Html)
    .map_err(WebError::from)
}

#[derive(Clone, Copy, Default, Display, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
enum Fit {
    Original,
    Width,
    Height,
    #[default]
    Both,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct MainParams {
    fit: Option<Fit>,
    search_text: Option<String>,
}

impl MainParams {
    fn fit(&self) -> Fit {
        self.fit.unwrap_or_default()
    }
}

#[derive(Template)]
#[template(path = "pages/post/main.html")]
struct MainTemplate {
    ctx: Ctx,
    active_tab: Tab,
    mode: Mode,
    post: PostInfo,
    prev_post: Option<PostInfo>,
    next_post: Option<PostInfo>,
    tag_categories: Vec<TagCategoryInfo>,
    params: MainParams,
}

impl MainTemplate {
    fn full_content_url(&self) -> Result<String, NotRequested> {
        self.post.content_url().map(|url| self.ctx.full_url(url))
    }
}

async fn main(ctx: Ctx, post_id: Path<i64>, Query(params): Query<MainParams>, mode: Mode) -> WebResult<Html> {
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
        Field::Comments,
        Field::Relations,
        Field::Score,
        Field::OwnScore,
        Field::OwnFavorite,
        Field::TagCount,
        Field::FavoriteCount,
    ]
    .into();

    let query = params.search_text.clone();
    let resource_params = Query(ResourceParams { query, fields });
    let Json(post) = api::post::get(ctx.clone(), post_id, resource_params.clone()).await?;
    let Json(neighbors) = api::post::get_neighbors(ctx.clone(), post_id, resource_params).await?;
    let tag_categories = web::tag_category::get_categories(ctx.clone()).await?;

    MainTemplate {
        ctx,
        active_tab: Tab::Post,
        mode,
        post,
        prev_post: neighbors.prev,
        next_post: neighbors.next,
        tag_categories,
        params,
    }
    .render()
    .map(Html)
    .map_err(WebError::from)
}

async fn view(ctx: Ctx, post_id: Path<i64>, params: Query<MainParams>) -> WebResult<Html> {
    main(ctx, post_id, params, Mode::View).await
}

async fn edit(ctx: Ctx, post_id: Path<i64>, params: Query<MainParams>) -> WebResult<Html> {
    main(ctx, post_id, params, Mode::Edit).await
}
