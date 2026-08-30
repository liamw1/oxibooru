use crate::api::error::ApiResult;
use crate::app::AppState;
use crate::config::{Action, RegexType};
use crate::extract::{Ctx, Form, HxRequest, Json, Offset, Path, Query, ResourceParams};
use crate::resource::field::Mask;
use crate::resource::tag::{Field, TagInfo};
use crate::resource::tag_category::TagCategoryInfo;
use crate::string::SmallString;
use crate::web::form::FormField;
use crate::web::form::tag::{DeleteForm, EditForm, Focus, Operation};
use crate::web::pager::{Page, Pager};
use crate::web::{Html, Message, Tab, WebError, WebResult};
use crate::{api, time, web};
use askama::Template;
use axum::response::{IntoResponse, Redirect, Response};
use axum::{Router, routing};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::{Deserialize, Serialize};
use server_macros::Deref;
use std::num::NonZeroU64;
use tokio::try_join;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/tags", routing::get(list))
        .route("/tag/{name}", routing::get(summary_tab))
        .route("/tag/{name}/edit", routing::get(edit_tab).post(edit_submit))
        .route("/tag/{name}/merge", routing::get(merge_tab))
        .route("/tag/{name}/delete", routing::get(delete_tab).post(delete_submit))
}

const DELETION_FLAG: &str = "tag-deleted";

const LIMIT: NonZeroU64 = NonZeroU64::new(50).unwrap();

const SUMMARY_FIELDS: [Field; 7] = [
    Field::Version,
    Field::Description,
    Field::Category,
    Field::Names,
    Field::Implications,
    Field::Suggestions,
    Field::Usages,
];

const DELETE_FIELDS: [Field; 3] = [Field::Version, Field::Names, Field::Usages];

fn edit_response_fields(ctx: &Ctx) -> Mask<Field> {
    let mut fields = [Field::Version, Field::Names].into();
    if ctx.has_privilege(Action::TagEditCategory) {
        fields |= Field::Category;
    }
    if ctx.has_privilege(Action::TagEditImplication) {
        fields |= Field::Implications;
    }
    if ctx.has_privilege(Action::TagEditSuggestion) {
        fields |= Field::Suggestions;
    }
    if ctx.has_privilege(Action::TagEditDescription) {
        fields |= Field::Description;
    }
    fields
}

async fn get_tag(ctx: Ctx, path: Path<SmallString>, fields: Mask<Field>) -> ApiResult<TagInfo> {
    let resource_params = Query(ResourceParams { query: None, fields });
    api::tag::get(ctx, path, resource_params).await.map(|Json(tag)| tag)
}

async fn get_tag_and_categories(
    ctx: Ctx,
    path: Path<SmallString>,
    fields: Mask<Field>,
) -> ApiResult<(TagInfo, Vec<TagCategoryInfo>)> {
    let tag_future = get_tag(ctx.clone(), path, fields);
    let categories_future = web::tag_category::get_categories(ctx.clone());
    try_join!(tag_future, categories_future)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Params {
    search_text: Option<String>,
}

impl Params {
    fn search_text(&self) -> &str {
        self.search_text.as_deref().unwrap_or("")
    }

    fn simplify(mut self) -> Self {
        if self.search_text.as_ref().is_some_and(String::is_empty) {
            self.search_text = None;
        }
        self
    }
}

#[derive(Template)]
#[template(path = "pages/tag/list.html")]
struct ListTemplate<'a> {
    ctx: Ctx,
    active_tab: Tab,
    tags: Vec<TagInfo>,
    categories: Vec<TagCategoryInfo>,
    pager: Pager<'a, Params>,
    params: &'a Params,
    message: Message,
}

async fn list(
    ctx: Ctx,
    jar: CookieJar,
    Query(params): Query<Params>,
    Query(offset): Query<Offset>,
) -> WebResult<Response> {
    let fields = [
        Field::CreationTime,
        Field::Category,
        Field::Names,
        Field::Implications,
        Field::Suggestions,
        Field::Usages,
    ]
    .into();

    let query = params.search_text.clone();
    let resource_params = Query(ResourceParams { query, fields });
    let page_params = Query(offset.to_page_params(LIMIT));
    let Json(response) = api::tag::list(ctx.clone(), resource_params, page_params).await?;
    let categories = web::tag_category::get_categories(ctx.clone()).await?;

    let params = params.simplify();
    let pager = Pager::build("tags", &params, page_params, response.total);

    let message = match jar.get("flash").map(Cookie::value) {
        Some(DELETION_FLAG) => Message::AfterDelete,
        _ => Message::None,
    };
    let jar = jar.remove(Cookie::build("flash").path("/")); // TODO: Adjust to base URL

    ListTemplate {
        ctx,
        active_tab: Tab::Tag,
        tags: response.results,
        categories,
        pager,
        params: &params,
        message,
    }
    .render()
    .map(|html| (jar, Html(html)).into_response())
    .map_err(WebError::from)
}

#[derive(PartialEq, Eq)]
enum TagTab {
    Summary,
    Edit,
    Merge,
    Delete,
}

struct TagSummaryInfo {
    ctx: Ctx,
    active_tab: Tab,
    active_tag_tab: TagTab,
    tag: TagInfo,
    categories: Vec<TagCategoryInfo>,
    message: Message,
}

impl TagSummaryInfo {
    async fn new(ctx: Ctx, path: Path<SmallString>, active_tag_tab: TagTab) -> WebResult<Self> {
        get_tag_and_categories(ctx.clone(), path, SUMMARY_FIELDS.into())
            .await
            .map(|(tag, categories)| Self {
                ctx,
                active_tab: Tab::Tag,
                active_tag_tab,
                tag,
                categories,
                message: Message::None,
            })
            .map_err(WebError::from)
    }
}

#[derive(Deref, Template)]
#[template(path = "pages/tag/summary.html")]
struct SummaryPageTemplate(TagSummaryInfo);

#[derive(Template)]
#[template(path = "pages/tag/summary.html", block = "tag")]
struct SummaryFragmentTemplate {
    active_tag_tab: TagTab,
    tag: TagInfo,
}

async fn summary_tab(ctx: Ctx, path: Path<SmallString>, hx: HxRequest) -> WebResult<Html> {
    if hx.full_page() {
        let page_info = TagSummaryInfo::new(ctx, path, TagTab::Summary).await?;
        SummaryPageTemplate(page_info).render()
    } else {
        let tag = get_tag(ctx.clone(), path, SUMMARY_FIELDS.into()).await?;
        SummaryFragmentTemplate {
            active_tag_tab: TagTab::Summary,
            tag,
        }
        .render()
    }
    .map(Html)
    .map_err(WebError::from)
}

struct TagEditInfo {
    ctx: Ctx,
    active_tab: Tab,
    active_tag_tab: TagTab,
    tag: EditForm,
    categories: Vec<TagCategoryInfo>,
    focus: Focus,
    message: Message,
}

impl TagEditInfo {
    async fn new(ctx: Ctx, path: Path<SmallString>) -> WebResult<Self> {
        let fields = edit_response_fields(&ctx);
        let (tag, categories) = get_tag_and_categories(ctx.clone(), path, fields).await?;
        Ok(Self {
            ctx,
            active_tab: Tab::Tag,
            active_tag_tab: TagTab::Edit,
            tag: EditForm::initialize(tag)?,
            categories,
            focus: Focus::None,
            message: Message::None,
        })
    }
}

#[derive(Deref, Template)]
#[template(path = "pages/tag/edit.html")]
struct EditPageTemplate(TagEditInfo);

#[derive(Deref, Template)]
#[template(path = "pages/tag/edit.html", block = "tag")]
struct EditFragmentTemplate(TagEditInfo);

async fn edit_tab(ctx: Ctx, path: Path<SmallString>, hx: HxRequest) -> WebResult<Html> {
    let page_info = TagEditInfo::new(ctx, path).await?;
    if hx.full_page() {
        EditPageTemplate(page_info).render()
    } else {
        EditFragmentTemplate(page_info).render()
    }
    .map(Html)
    .map_err(WebError::from)
}

async fn edit_submit(ctx: Ctx, path: Path<SmallString>, hx: HxRequest, Form(form): Form<EditForm>) -> WebResult<Html> {
    let (form, focus, message) = match form.operation {
        Operation::Init => unreachable!(),
        Operation::Auto => form.auto_modify(ctx.clone()).await?,
        Operation::AddImplication => form.with_new_implications(ctx.clone()).await?,
        Operation::AddSuggestion => form.with_new_suggestions(ctx.clone()).await?,
        Operation::RemoveImplication(index) => form.with_implication_removed(index),
        Operation::RemoveSuggestion(index) => form.with_suggestion_removed(index),
        Operation::Save => {
            let focus = Focus::None;
            let fields = edit_response_fields(&ctx);
            match api::tag::update(ctx.clone(), path.clone(), Query(fields.into()), Json(form.to_body())).await {
                Ok(Json(tag)) => (EditForm::initialize(tag)?, focus, Message::Success),
                Err(err) => (form, focus, Message::Error(err)),
            }
        }
    };

    let categories = web::tag_category::get_categories(ctx.clone()).await?;
    let page_info = TagEditInfo {
        ctx,
        active_tab: Tab::Tag,
        active_tag_tab: TagTab::Edit,
        tag: form,
        categories,
        focus,
        message,
    };
    if hx.full_page() {
        EditPageTemplate(page_info).render()
    } else {
        EditFragmentTemplate(page_info).render()
    }
    .map(Html)
    .map_err(WebError::from)
}

#[derive(Deref, Template)]
#[template(path = "pages/tag/merge.html")]
struct MergePageTemplate(TagSummaryInfo);

#[derive(Template)]
#[template(path = "pages/tag/merge.html", block = "tag")]
struct MergeFragmentTemplate {
    ctx: Ctx,
    active_tag_tab: TagTab,
    tag: TagInfo,
    message: Message,
}

async fn merge_tab(ctx: Ctx, path: Path<SmallString>, hx: HxRequest) -> WebResult<Html> {
    if hx.full_page() {
        let page_info = TagSummaryInfo::new(ctx, path, TagTab::Merge).await?;
        MergePageTemplate(page_info).render()
    } else {
        let tag = get_tag(ctx.clone(), path, SUMMARY_FIELDS.into()).await?;
        MergeFragmentTemplate {
            ctx,
            active_tag_tab: TagTab::Merge,
            tag,
            message: Message::None,
        }
        .render()
    }
    .map(Html)
    .map_err(WebError::from)
}

struct TagDeleteInfo {
    ctx: Ctx,
    active_tab: Tab,
    active_tag_tab: TagTab,
    tag: DeleteForm,
    categories: Vec<TagCategoryInfo>,
    message: Message,
}

#[derive(Deref, Template)]
#[template(path = "pages/tag/delete.html")]
struct DeletePageTemplate(TagDeleteInfo);

#[derive(Template)]
#[template(path = "pages/tag/delete.html", block = "tag")]
struct DeleteFragmentTemplate {
    active_tag_tab: TagTab,
    tag: DeleteForm,
    message: Message,
}

async fn delete_tab(ctx: Ctx, path: Path<SmallString>, hx: HxRequest) -> WebResult<Html> {
    if hx.full_page() {
        let (tag, categories) = get_tag_and_categories(ctx.clone(), path, DELETE_FIELDS.into()).await?;
        let page_info = TagDeleteInfo {
            ctx,
            active_tab: Tab::Tag,
            active_tag_tab: TagTab::Delete,
            tag: DeleteForm::initialize(tag)?,
            categories,
            message: Message::None,
        };
        DeletePageTemplate(page_info).render()
    } else {
        let tag = get_tag(ctx.clone(), path, DELETE_FIELDS.into()).await?;
        DeleteFragmentTemplate {
            active_tag_tab: TagTab::Delete,
            tag: DeleteForm::initialize(tag)?,
            message: Message::None,
        }
        .render()
    }
    .map(Html)
    .map_err(WebError::from)
}

async fn delete_submit(
    ctx: Ctx,
    path: Path<SmallString>,
    hx: HxRequest,
    jar: CookieJar,
    Form(form): Form<DeleteForm>,
) -> WebResult<Response> {
    match api::tag::delete(ctx.clone(), path.clone(), Json(form.to_body())).await {
        Ok(Json(())) => {
            let flash = Cookie::build(("flash", DELETION_FLAG))
                .path("/") // TODO: Adjust to base URL
                .http_only(true)
                .same_site(SameSite::Strict)
                .build();
            let jar = jar.add(flash);

            // TODO: Generate using base URL
            let location = "/tags";
            Ok(if hx.full_page() {
                (jar, Redirect::to(location)).into_response()
            } else {
                (jar, [("HX-Redirect", location)], "").into_response()
            })
        }
        Err(err) => {
            let message = Message::Error(err);
            if hx.full_page() {
                let categories = web::tag_category::get_categories(ctx.clone()).await?;
                let page_info = TagDeleteInfo {
                    ctx,
                    active_tab: Tab::Tag,
                    active_tag_tab: TagTab::Delete,
                    tag: form,
                    categories,
                    message,
                };
                DeletePageTemplate(page_info).render()
            } else {
                DeleteFragmentTemplate {
                    active_tag_tab: TagTab::Delete,
                    tag: form,
                    message,
                }
                .render()
            }
            .map(Html)
            .map(Html::into_response)
            .map_err(WebError::from)
        }
    }
}
