use crate::api::error::ApiResult;
use crate::api::post::PostNeighbors;
use crate::app::AppState;
use crate::config::Action;
use crate::extract::{Ctx, Form, HxRequest, Json, JsonOrMultipart, Offset, Path, Query, ResourceParams};
use crate::model::enums::{PostFlag, PostSafety, PostType, Rating};
use crate::resource::NotRequested;
use crate::resource::field::Mask;
use crate::resource::pool_category::PoolCategoryInfo;
use crate::resource::post::{Field, Mode, PostInfo};
use crate::resource::tag_category::TagCategoryInfo;
use crate::web::form::FormField;
use crate::web::form::post::{EditMode, EditPathForm, Focus, Operation, SafetyToggleForm};
use crate::web::pager::{Page, Pager};
use crate::web::{Html, Message, Tab, WebError, WebResult};
use crate::{api, time, unit, web};
use askama::Template;
use axum::response::{IntoResponse, Response};
use axum::{Router, routing};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use serde::{Deserialize, Serialize};
use server_macros::Deref;
use std::num::NonZeroU64;
use std::ops::{Deref, DerefMut};
use strum::{Display, IntoEnumIterator};
use tokio::try_join;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/posts", routing::get(gallery))
        .route("/posts/safety", routing::post(toggle_safety))
        .route("/post/{post_id}", routing::get(view))
        .route("/post/{post_id}/edit", routing::get(edit).post(edit_submit))
}

const SAFETY_COOKIE: &str = "safety";

const LIMIT: NonZeroU64 = NonZeroU64::new(42).unwrap();

const SAVED_MESSAGE: &str = "Post saved.";

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

#[derive(Clone, Copy, Serialize, Deserialize)]
struct SafetyFilter {
    enabled_safeties: Mask<PostSafety>,
}

impl SafetyFilter {
    fn from_jar(jar: &CookieJar) -> Self {
        jar.get(SAFETY_COOKIE)
            .and_then(|cookie| serde_urlencoded::from_str(cookie.value()).ok())
            .unwrap_or_default()
    }

    fn to_cookie(self) -> Cookie<'static> {
        let value = serde_urlencoded::to_string(self).expect("SafetyFilter is urlencodable");
        Cookie::build((SAFETY_COOKIE, value))
            .path("/") // TODO: Adjust to base URL
            .http_only(true)
            .same_site(SameSite::Strict)
            .permanent()
            .build()
    }

    fn query_filter(self) -> Option<&'static str> {
        match (self[PostSafety::Safe], self[PostSafety::Sketchy], self[PostSafety::Unsafe]) {
            (false, false, false) => Some("-safety:safe,sketchy,unsafe"),
            (false, false, true) => Some("safety:unsafe"),
            (false, true, false) => Some("safety:sketchy"),
            (false, true, true) => Some("-safety:safe"),
            (true, false, false) => Some("safety:safe"),
            (true, false, true) => Some("-safety:sketchy"),
            (true, true, false) => Some("-safety:unsafe"),
            (true, true, true) => None,
        }
    }
}

impl Default for SafetyFilter {
    fn default() -> Self {
        Self {
            enabled_safeties: Mask::none() | PostSafety::Safe | PostSafety::Sketchy,
        }
    }
}

impl Deref for SafetyFilter {
    type Target = Mask<PostSafety>;
    fn deref(&self) -> &Self::Target {
        &self.enabled_safeties
    }
}

impl DerefMut for SafetyFilter {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.enabled_safeties
    }
}

async fn get_post(ctx: Ctx, path: Path<i64>, params: &MainParams, fields: Mask<Field>) -> ApiResult<PostInfo> {
    let query = params.query.clone();
    let resource_params = Query(ResourceParams { query, fields });
    api::post::get(ctx, path, resource_params).await.map(|Json(post)| post)
}

async fn get_neighbors_and_categories(
    ctx: Ctx,
    path: Path<i64>,
    params: &MainParams,
    fields: Mask<Field>,
) -> ApiResult<(PostNeighbors, Vec<TagCategoryInfo>, Vec<PoolCategoryInfo>)> {
    let query = params.query.clone();
    let resource_params = Query(ResourceParams { query, fields });

    let neighbors_future = api::post::get_neighbors(ctx.clone(), path, resource_params);
    let tag_categories_future = web::tag_category::get_categories(ctx.clone());
    let pool_categories_future = web::pool_category::get_categories(ctx.clone());
    try_join!(neighbors_future, tag_categories_future, pool_categories_future)
        .map(|(Json(neighbors), tag_categories, pool_categories)| (neighbors, tag_categories, pool_categories))
}

async fn get_posts_and_categories(
    ctx: Ctx,
    path: Path<i64>,
    params: &MainParams,
    fields: Mask<Field>,
) -> ApiResult<(PostInfo, PostNeighbors, Vec<TagCategoryInfo>, Vec<PoolCategoryInfo>)> {
    let post_future = get_post(ctx.clone(), path, params, fields);
    let neighbors_and_categories_future = get_neighbors_and_categories(ctx, path, params, fields);
    try_join!(post_future, neighbors_and_categories_future)
        .map(|(post, (neighbors, tag_categories, pool_categories))| (post, neighbors, tag_categories, pool_categories))
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ListParams {
    edit: Option<EditMode>,
    search_text: Option<String>,
}

impl ListParams {
    fn search_text(&self) -> &str {
        self.search_text.as_deref().unwrap_or("")
    }

    fn query(&self, safety: SafetyFilter) -> Option<String> {
        match (self.search_text.clone(), safety.query_filter()) {
            (Some(text), Some(filter)) => Some(format!("{text} {filter}")),
            (None, filter) => filter.map(str::to_owned),
            (text, None) => text,
        }
    }

    fn simplify(mut self) -> Self {
        if self.search_text.as_ref().is_some_and(String::is_empty) {
            self.search_text = None;
        }
        self
    }

    fn to_main_params(&self, safety: SafetyFilter) -> MainParams {
        MainParams {
            query: self.query(safety),
            fit: None,
        }
    }
}

impl From<SafetyToggleForm> for ListParams {
    fn from(value: SafetyToggleForm) -> Self {
        Self {
            edit: value.edit,
            search_text: value.search_text,
        }
        .simplify()
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
    safety: SafetyFilter,
    message: Message,
}

async fn gallery(
    ctx: Ctx,
    Query(params): Query<ListParams>,
    Query(offset): Query<Offset>,
    jar: CookieJar,
) -> WebResult<Response> {
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

    let safety = SafetyFilter::from_jar(&jar);
    let query = params.query(safety);
    let resource_params = Query(ResourceParams { query, fields });
    let page_params = Query(offset.to_page_params(LIMIT));
    let Json(response) = api::post::list(ctx.clone(), resource_params, page_params).await?;

    let params = params.simplify();
    let pager = Pager::build("posts", &params, page_params, response.total);
    let (jar, message) = web::redirect_message(jar);
    GalleryTemplate {
        ctx,
        active_tab: Tab::Post,
        posts: response.results,
        pager,
        params: &params,
        safety,
        message,
    }
    .render()
    .map(|html| (jar, Html(html)).into_response())
    .map_err(WebError::from)
}

async fn toggle_safety(hx: HxRequest, jar: CookieJar, Form(form): Form<SafetyToggleForm>) -> WebResult<Response> {
    let mut safety = SafetyFilter::from_jar(&jar);
    safety.toggle(form.toggle);
    let jar = jar.add(safety.to_cookie());

    let url = web::url("/posts", &ListParams::from(form))?;
    Ok(web::plain_redirect(&url, &hx, jar))
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
    query: Option<String>,
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
    focus: Focus,
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
                focus: Focus::None,
                message: Message::None,
            })
    }
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
        ViewFragmentTemplate { ctx, post, params }.render()
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
    focus: Focus,
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
            focus: Focus::None,
            message,
        };
        EditTemplate(page_info).render()
    } else {
        let post = get_post(ctx.clone(), path, &params, fields).await?;
        EditFragmentTemplate {
            ctx,
            post: EditPathForm::initialize(post)?,
            params,
            focus: Focus::None,
            message,
        }
        .render()
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
    let fields = Mask::from(VIEW_FIELDS) | Field::Version;
    let (updated_form, focus, message) = match form.operation {
        Operation::Init => unreachable!(),
        Operation::Auto => form.auto_modify(ctx.clone()).await?,
        Operation::AddTag => form.with_new_tags(ctx.clone()).await?,
        Operation::AddPool => form.with_new_pools(ctx.clone()).await?,
        Operation::RemoveTag(index) => form.with_tag_removed(index),
        Operation::RemovePool(index) => form.with_pool_removed(index),
        Operation::Save => {
            let focus = Focus::None;
            let body = form.to_body().map(JsonOrMultipart::Json)?;
            match api::post::update(ctx.clone(), form.path(), Query(fields.into()), body).await {
                Ok(Json(post)) => {
                    if !hx.htmx() {
                        let new_url = post.url(Mode::Edit, &params)?;
                        return Ok(web::redirect(&new_url, &hx, jar, SAVED_MESSAGE));
                    }
                    (EditPathForm::initialize(post)?, focus, Message::Success(SAVED_MESSAGE.into()))
                }
                Err(err) => (form, focus, Message::Error(err)),
            }
        }
    };

    if hx.full_page() {
        let (neighbors, tag_categories, pool_categories) =
            get_neighbors_and_categories(ctx.clone(), updated_form.path(), &params, fields).await?;
        let page_info = PostPage {
            ctx,
            active_tab: Tab::Post,
            mode: Mode::Edit,
            post: updated_form,
            prev_post: neighbors.prev,
            next_post: neighbors.next,
            tag_categories,
            pool_categories,
            params,
            focus,
            message,
        };
        EditTemplate(page_info).render()
    } else {
        EditFragmentTemplate {
            ctx,
            post: updated_form,
            params,
            focus,
            message,
        }
        .render()
    }
    .map(Html)
    .map(Html::into_response)
    .map_err(WebError::from)
}
