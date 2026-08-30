use crate::api::error::ApiResult;
use crate::app::AppState;
use crate::config::{Action, RegexType};
use crate::extract::{Ctx, Form, HxRequest, Json, Offset, Path, Query, ResourceParams};
use crate::resource::field::Mask;
use crate::resource::tag::{Field, TagInfo};
use crate::resource::tag_category::TagCategoryInfo;
use crate::string::SmallString;
use crate::web::form::FormField;
use crate::web::form::tag::{EditForm, Focus, Operation};
use crate::web::pager::{Page, Pager};
use crate::web::{Html, Message, Tab, WebError, WebResult};
use crate::{api, time, web};
use askama::Template;
use axum::{Router, routing};
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
        .route("/tag/{name}/delete", routing::get(delete_tab))
}

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
}

async fn list(ctx: Ctx, Query(params): Query<Params>, Query(offset): Query<Offset>) -> WebResult<Html> {
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

    ListTemplate {
        ctx,
        active_tab: Tab::Tag,
        tags: response.results,
        categories,
        pager,
        params: &params,
    }
    .render()
    .map(Html)
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
}

impl TagSummaryInfo {
    async fn new(ctx: Ctx, path: Path<SmallString>, active_tag_tab: TagTab) -> WebResult<Self> {
        let fields = SUMMARY_FIELDS.into();
        let tag_future = get_tag(ctx.clone(), path, fields);
        let categories_future = web::tag_category::get_categories(ctx.clone());
        try_join!(tag_future, categories_future)
            .map(|(tag, categories)| Self {
                ctx,
                active_tab: Tab::Tag,
                active_tag_tab,
                tag,
                categories,
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
    let fields = SUMMARY_FIELDS.into();

    if hx.full_page() {
        let page_info = TagSummaryInfo::new(ctx, path, TagTab::Summary).await?;
        SummaryPageTemplate(page_info).render()
    } else {
        let tag = get_tag(ctx.clone(), path, fields).await?;
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
        let tag_future = get_tag(ctx.clone(), path, fields);
        let categories_future = web::tag_category::get_categories(ctx.clone());
        let (tag, categories) = try_join!(tag_future, categories_future)?;
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
    let (tag, focus, message) = match form.operation() {
        Operation::Init => unreachable!(),
        Operation::Auto => form.auto_modify(ctx.clone()).await?,
        Operation::AddImplication => form.with_new_implications(ctx.clone()).await?,
        Operation::AddSuggestion => form.with_new_suggestions(ctx.clone()).await?,
        Operation::RemoveImplication(index) => form.with_implication_removed(index),
        Operation::RemoveSuggestion(index) => form.with_suggestion_removed(index),
        Operation::Save => {
            let focus = Focus::None;
            let fields = edit_response_fields(&ctx);
            let query = Query(ResourceParams { query: None, fields });
            let json = Json(form.to_body());
            match api::tag::update(ctx.clone(), path.clone(), query, json).await {
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
        tag,
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
}

async fn merge_tab(ctx: Ctx, path: Path<SmallString>, hx: HxRequest) -> WebResult<Html> {
    let fields = SUMMARY_FIELDS.into();

    if hx.full_page() {
        let page_info = TagSummaryInfo::new(ctx, path, TagTab::Merge).await?;
        MergePageTemplate(page_info).render()
    } else {
        let tag = get_tag(ctx.clone(), path, fields).await?;
        MergeFragmentTemplate {
            ctx,
            active_tag_tab: TagTab::Merge,
            tag,
        }
        .render()
    }
    .map(Html)
    .map_err(WebError::from)
}

#[derive(Deref, Template)]
#[template(path = "pages/tag/delete.html")]
struct DeletePageTemplate(TagSummaryInfo);

#[derive(Template)]
#[template(path = "pages/tag/delete.html", block = "tag")]
struct DeleteFragmentTemplate {
    active_tag_tab: TagTab,
    tag: TagInfo,
}

async fn delete_tab(ctx: Ctx, path: Path<SmallString>, hx: HxRequest) -> WebResult<Html> {
    let fields = SUMMARY_FIELDS.into();

    if hx.full_page() {
        let page_info = TagSummaryInfo::new(ctx, path, TagTab::Delete).await?;
        DeletePageTemplate(page_info).render()
    } else {
        let tag = get_tag(ctx.clone(), path, fields).await?;
        DeleteFragmentTemplate {
            active_tag_tab: TagTab::Delete,
            tag,
        }
        .render()
    }
    .map(Html)
    .map_err(WebError::from)
}
