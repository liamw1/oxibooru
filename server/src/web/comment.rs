use crate::api::comment::CommentUpdateBody;
use crate::app::AppState;
use crate::config::Action;
use crate::extract::{Ctx, Form, HxRequest, Json, Offset, Path, Query, ResourceParams};
use crate::model::enums::Rating;
use crate::resource::comment::{CommentInfo, Field as CommentField};
use crate::resource::post::{Field, Mode, PostInfo};
use crate::time::DateTime;
use crate::web::cookie::safety::SafetyFilter;
use crate::web::form::comment::{EditForm, Operation};
use crate::web::pager::{Page, Pager};
use crate::web::post::MainParams;
use crate::web::{Html, Message, Tab, WebError, WebResult, markdown};
use crate::{api, time, web};
use askama::Template;
use axum::response::{IntoResponse, Response};
use axum::{Router, routing};
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize, Serializer};
use std::borrow::Borrow;
use std::num::NonZeroU64;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EditTab {
    Write,
    Preview,
}

pub struct Draft {
    pub comment_id: i64,
    pub post_id: i64,
    pub text: String,
    pub version: DateTime,
    pub tab: EditTab,
    pub message: Message,
}

impl Draft {
    fn new(comment: &CommentInfo) -> WebResult<Self> {
        Ok(Self {
            comment_id: comment.id()?,
            post_id: comment.post_id()?,
            text: comment.text()?.to_string(),
            version: comment.version()?,
            tab: EditTab::Write,
            message: Message::None,
        })
    }
}

#[derive(Deserialize)]
#[serde(from = "LocationParams")]
pub enum Location {
    Post { mode: Mode, params: MainParams },
    Comments { offset: Option<u64> },
}

impl Location {
    pub fn post(mode: impl Borrow<Mode>, params: &MainParams) -> Self {
        Self::Post {
            mode: *mode.borrow(),
            params: params.clone(),
        }
    }

    pub fn url(&self, comment: &CommentInfo) -> WebResult<String> {
        match self {
            Self::Post { mode, params } => {
                let post_id = comment.post_id()?;
                match mode {
                    Mode::View => web::post_url(post_id, params),
                    Mode::Edit => web::post_edit_url(post_id, params),
                }
                .map_err(WebError::from)
            }
            &Self::Comments { offset } => Ok(match offset {
                Some(0) | None => String::from("comments"),
                Some(offset) => format!("comments?offset={offset}"),
            }),
        }
    }

    fn redirect_url(&self, comment: &CommentInfo) -> WebResult<String> {
        let comment_id = comment.id()?;
        // Absolute because browsers resolve redirects against the request URL, not <base href>
        // TODO: Generalize
        self.url(comment).map(|url| format!("/{url}#comment-{comment_id}"))
    }

    async fn render(self, ctx: Ctx, jar: &CookieJar, comment: &CommentInfo, draft: Draft) -> WebResult<Html> {
        let post_id = comment.post_id()?;
        match self {
            Self::Post { mode, params } => web::post::render_with_draft(ctx, post_id, mode, params, draft).await,
            Self::Comments { offset } => render_list(ctx, Offset::new(offset), jar, Some(draft)).await,
        }
    }
}

impl From<LocationParams> for Location {
    fn from(value: LocationParams) -> Self {
        match value.on {
            LocationKind::Post => Self::Post {
                mode: Mode::View,
                params: value.params,
            },
            LocationKind::PostEdit => Self::Post {
                mode: Mode::Edit,
                params: value.params,
            },
            LocationKind::Comments => Self::Comments { offset: value.offset },
        }
    }
}

impl Serialize for Location {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let (on, offset, params) = match self {
            Self::Post {
                mode: Mode::View,
                params,
            } => (LocationKind::Post, None, Some(params)),
            Self::Post {
                mode: Mode::Edit,
                params,
            } => (LocationKind::PostEdit, None, Some(params)),
            &Self::Comments { offset } => (LocationKind::Comments, offset, None),
        };

        let params = params.cloned().unwrap_or_default();
        LocationParams { on, offset, params }.serialize(serializer)
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/comments", routing::get(list))
        .route("/comment/{comment_id}", routing::get(view))
        .route("/comment/{comment_id}/edit", routing::get(edit).post(edit_submit))
}

const LIMIT: NonZeroU64 = NonZeroU64::new(10).unwrap();

const COMMENT_FIELDS: [CommentField; 8] = [
    CommentField::Version,
    CommentField::Id,
    CommentField::PostId,
    CommentField::Text,
    CommentField::CreationTime,
    CommentField::User,
    CommentField::Score,
    CommentField::OwnScore,
];

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum LocationKind {
    #[default]
    Post,
    PostEdit,
    Comments,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct LocationParams {
    #[serde(default)]
    on: LocationKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    offset: Option<u64>,
    #[serde(flatten)]
    params: MainParams,
}

#[derive(Template)]
#[template(path = "pages/comment/list.html")]
struct ListTemplate<'a> {
    ctx: Ctx,
    active_tab: Tab,
    posts: Vec<PostInfo>,
    pager: Pager<'a, ()>,
    location: Location,
    draft: Option<Draft>,
}

async fn render_list(ctx: Ctx, offset: Offset, jar: &CookieJar, draft: Option<Draft>) -> WebResult<Html> {
    const BASE_QUERY: &str = "sort:comment-date comment-count:1..";
    const FIELDS: [Field; 3] = [Field::Id, Field::ThumbnailUrl, Field::Comments];

    let list_query = match SafetyFilter::from_jar(jar).query_filter() {
        Some(filter) => format!("{BASE_QUERY} {filter}"),
        None => BASE_QUERY.to_owned(),
    };
    let resource_params = Query(ResourceParams {
        query: Some(list_query),
        fields: FIELDS.into(),
    });
    let page_params = Query(offset.to_page_params(LIMIT));
    let Json(response) = api::post::list(ctx.clone(), resource_params, page_params).await?;
    let mut posts = response.results;

    // The post may have moved off this page, or not be on the list at all
    if let Some(draft) = &draft
        && !posts.iter().any(|post| post.id().is_ok_and(|id| id == draft.post_id))
    {
        let Json(post) = api::post::get(ctx.clone(), Path(draft.post_id), Query(FIELDS.into())).await?;
        posts.push(post);
    }

    let pager = Pager::build("comments", &(), page_params, response.total);
    ListTemplate {
        ctx,
        active_tab: Tab::Comment,
        posts,
        pager,
        location: Location::Comments {
            offset: page_params.offset.filter(|&offset| offset > 0),
        },
        draft,
    }
    .render()
    .map(Html)
    .map_err(WebError::from)
}

async fn list(ctx: Ctx, Query(offset): Query<Offset>, jar: CookieJar) -> WebResult<Html> {
    render_list(ctx, offset, &jar, None).await
}

#[derive(Template)]
#[template(path = "partials/comment.html")]
struct CommentTemplate {
    ctx: Ctx,
    comment: CommentInfo,
    draft: Option<Draft>,
    location: Location,
}

async fn render(
    hx: &HxRequest,
    ctx: Ctx,
    jar: &CookieJar,
    location: Location,
    comment: CommentInfo,
    draft: Option<Draft>,
) -> WebResult<Response> {
    if hx.full_page()
        && let Some(draft) = draft
    {
        return location
            .render(ctx, jar, &comment, draft)
            .await
            .map(IntoResponse::into_response);
    }

    CommentTemplate {
        ctx,
        comment,
        draft,
        location,
    }
    .render()
    .map(Html)
    .map(IntoResponse::into_response)
    .map_err(WebError::from)
}

async fn get_comment(ctx: Ctx, comment_id: i64) -> WebResult<CommentInfo> {
    let Json(comment) = api::comment::get(ctx, Path(comment_id), Query(COMMENT_FIELDS.into())).await?;
    Ok(comment)
}

async fn view(
    ctx: Ctx,
    Path(comment_id): Path<i64>,
    Query(location): Query<Location>,
    hx: HxRequest,
    jar: CookieJar,
) -> WebResult<Response> {
    let comment = get_comment(ctx.clone(), comment_id).await?;
    if hx.full_page() {
        let redirect_url = location.redirect_url(&comment)?;
        return Ok(web::plain_redirect(&redirect_url, hx, jar));
    }
    render(&hx, ctx, &jar, location, comment, None).await
}

async fn edit(
    ctx: Ctx,
    Path(comment_id): Path<i64>,
    Query(location): Query<Location>,
    hx: HxRequest,
    jar: CookieJar,
) -> WebResult<Response> {
    let comment = get_comment(ctx.clone(), comment_id).await?;
    let draft = Draft::new(&comment)?;
    render(&hx, ctx, &jar, location, comment, Some(draft)).await
}

async fn edit_submit(
    ctx: Ctx,
    Path(comment_id): Path<i64>,
    Query(location): Query<Location>,
    hx: HxRequest,
    jar: CookieJar,
    Form(form): Form<EditForm>,
) -> WebResult<Response> {
    let (tab, message) = match form.operation {
        Operation::Write => (EditTab::Write, Message::None),
        Operation::Preview => (EditTab::Preview, Message::None),
        Operation::Save => {
            let params = Query(COMMENT_FIELDS.into());
            let body = Json(CommentUpdateBody {
                version: form.version,
                text: form.text.clone(),
            });
            match api::comment::update(ctx.clone(), Path(comment_id), params, body).await {
                Ok(Json(comment)) => {
                    if hx.full_page() {
                        let redirect_url = location.redirect_url(&comment)?;
                        return Ok(web::plain_redirect(&redirect_url, hx, jar));
                    }
                    return render(&hx, ctx, &jar, location, comment, None).await;
                }
                Err(err) => (EditTab::Write, Message::Error(err)),
            }
        }
    };

    let comment = get_comment(ctx.clone(), comment_id).await?;

    // Keep the original version so conflicting edits are still detected on save
    let draft = Draft {
        comment_id,
        post_id: comment.post_id()?,
        text: form.text,
        version: form.version,
        tab,
        message,
    };
    render(&hx, ctx, &jar, location, comment, Some(draft)).await
}
