use crate::api::error::ApiResult;
use crate::api::post::PostNeighbors;
use crate::app::AppState;
use crate::config::Action;
use crate::extract::{Ctx, HxRequest, Json, Offset, Path, Query, ResourceParams};
use crate::model::enums::{PostFlag, PostSafety, PostType, Rating};
use crate::resource::NotRequested;
use crate::resource::field::Mask;
use crate::resource::pool_category::PoolCategoryInfo;
use crate::resource::post::{Field, Mode, PostInfo};
use crate::resource::tag_category::TagCategoryInfo;
use crate::web::form::FormField;
use crate::web::form::post::{EditPathForm, Focus, Operation};
use crate::web::pager::{Page, Pager};
use crate::web::{Html, Message, Tab, WebError, WebResult};
use crate::{api, time, unit, web};
use askama::Template;
use axum::response::{IntoResponse, Response};
use axum::{Router, routing};
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};
use server_macros::Deref;
use std::num::NonZeroU64;
use strum::{Display, IntoEnumIterator};
use tokio::try_join;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/posts", routing::get(gallery))
        .route("/post/{post_id}", routing::get(view))
        .route("/post/{post_id}/edit", routing::get(edit).post(edit_submit))
}

const SAFE_DEFAULT: bool = true;
const SKETCHY_DEFAULT: bool = true;
const UNSAFE_DEFAULT: bool = false;

const LIMIT: NonZeroU64 = NonZeroU64::new(42).unwrap();

const VIEW_FIELDS: [Field; 24] = [
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
    Field::Pools,
    Field::Notes,
    Field::Score,
    Field::OwnScore,
    Field::OwnFavorite,
    Field::FavoriteCount,
];

async fn get_post(ctx: Ctx, path: Path<i64>, params: &MainParams, fields: Mask<Field>) -> ApiResult<PostInfo> {
    let query = params.search_text.clone();
    let resource_params = Query(ResourceParams { query, fields });
    api::post::get(ctx, path, resource_params).await.map(|Json(post)| post)
}

async fn get_posts_and_categories(
    ctx: Ctx,
    path: Path<i64>,
    params: &MainParams,
    fields: Mask<Field>,
) -> ApiResult<(PostInfo, PostNeighbors, Vec<TagCategoryInfo>, Vec<PoolCategoryInfo>)> {
    let query = params.search_text.clone();
    let resource_params = Query(ResourceParams { query, fields });

    let post_future = api::post::get(ctx.clone(), path, resource_params.clone());
    let neighbors_future = api::post::get_neighbors(ctx.clone(), path, resource_params);
    let tag_categories_future = web::tag_category::get_categories(ctx.clone());
    let pool_categories_future = web::pool_category::get_categories(ctx.clone());
    try_join!(post_future, neighbors_future, tag_categories_future, pool_categories_future).map(
        |(Json(post), Json(neighbors), tag_categories, pool_categories)| {
            (post, neighbors, tag_categories, pool_categories)
        },
    )
}

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

struct PostPage<T> {
    ctx: Ctx,
    active_tab: Tab,
    mode: Mode,
    post: T,
    prev_post: Option<PostInfo>,
    next_post: Option<PostInfo>,
    tag_categories: Vec<TagCategoryInfo>,
    pool_categories: Vec<PoolCategoryInfo>,
    params: MainParams,
    message: Message,
}

impl PostPage<PostInfo> {
    async fn new(ctx: Ctx, path: Path<i64>, params: MainParams, mode: Mode) -> ApiResult<Self> {
        get_posts_and_categories(ctx.clone(), path, &params, VIEW_FIELDS.into())
            .await
            .map(|(post, neighbors, tag_categories, pool_categories)| Self {
                ctx,
                active_tab: Tab::Post,
                mode,
                post,
                prev_post: neighbors.prev,
                next_post: neighbors.next,
                tag_categories,
                pool_categories,
                params,
                message: Message::None,
            })
    }
}

#[derive(Template)]
#[template(path = "partials/post/edit_toggle.html")]
struct EditToggleTemplate<'a> {
    ctx: &'a Ctx,
    post: &'a PostInfo,
    params: &'a MainParams,
    mode: Mode,
    oob: bool,
}

#[derive(Deref, Template)]
#[template(path = "pages/post/view.html")]
struct ViewTemplate(PostPage<PostInfo>);

impl ViewTemplate {
    fn full_content_url(&self) -> Result<String, NotRequested> {
        self.post.content_url().map(|url| self.ctx.full_url(url))
    }
}

#[derive(Template)]
#[template(path = "pages/post/view.html", block = "sidebar_content")]
struct ViewFragmentTemplate {
    ctx: Ctx,
    post: PostInfo,
    params: MainParams,
}

impl ViewFragmentTemplate {
    fn full_content_url(&self) -> Result<String, NotRequested> {
        self.post.content_url().map(|url| self.ctx.full_url(url))
    }
}

async fn view(ctx: Ctx, path: Path<i64>, Query(params): Query<MainParams>, hx: HxRequest) -> WebResult<Html> {
    if hx.full_page() {
        let page_info = PostPage::new(ctx, path, params, Mode::View).await?;
        ViewTemplate(page_info).render()
    } else {
        let post = get_post(ctx.clone(), path, &params, VIEW_FIELDS.into()).await?;

        let edit_toggle = EditToggleTemplate {
            ctx: &ctx,
            post: &post,
            params: &params,
            mode: Mode::View,
            oob: true,
        }
        .render()?;
        ViewFragmentTemplate { ctx, post, params }
            .render()
            .map(|sidebar| sidebar + &edit_toggle)
    }
    .map(Html)
    .map_err(WebError::from)
}

#[derive(Deref, Template)]
#[template(path = "pages/post/edit.html")]
struct EditTemplate(PostPage<EditPathForm>);

#[derive(Template)]
#[template(path = "pages/post/edit.html", block = "sidebar_content")]
struct EditFragmentTemplate {
    ctx: Ctx,
    post: EditPathForm,
    params: MainParams,
    message: Message,
}

async fn edit(
    ctx: Ctx,
    path: Path<i64>,
    Query(params): Query<MainParams>,
    hx: HxRequest,
    jar: CookieJar,
) -> WebResult<Response> {
    let fields = Mask::from(VIEW_FIELDS) | Field::Version;
    let (jar, message) = web::redirect_message(jar);
    if hx.full_page() {
        let (post, neighbors, tag_categories, pool_categories) =
            get_posts_and_categories(ctx.clone(), path, &params, fields).await?;
        let page_info = PostPage {
            ctx,
            active_tab: Tab::Post,
            mode: Mode::Edit,
            post: EditPathForm::initialize(post)?,
            prev_post: neighbors.prev,
            next_post: neighbors.next,
            tag_categories,
            pool_categories,
            params,
            message,
        };
        EditTemplate(page_info).render()
    } else {
        let post = get_post(ctx.clone(), path, &params, fields).await?;

        let edit_toggle = EditToggleTemplate {
            ctx: &ctx,
            post: &post,
            params: &params,
            mode: Mode::Edit,
            oob: true,
        }
        .render()?;
        EditFragmentTemplate {
            ctx,
            post: EditPathForm::initialize(post)?,
            params,
            message,
        }
        .render()
        .map(|sidebar| sidebar + &edit_toggle)
    }
    .map(|html| (jar, Html(html)).into_response())
    .map_err(WebError::from)
}

async fn edit_submit(
    ctx: Ctx,
    Query(params): Query<MainParams>,
    hx: HxRequest,
    jar: CookieJar,
    form: EditPathForm,
) -> WebResult<Response> {
    let (updated_form, focus, message) = match form.operation {
        Operation::Init => unreachable!(),
        Operation::Auto => todo!(),
        Operation::AddTag => todo!(),
        Operation::AddPool => todo!(),
        Operation::RemoveTag(index) => form.with_tag_removed(index),
        Operation::RemovePool(index) => form.with_pool_removed(index),
        Operation::Save => todo!(),
    };

    todo!()
}
