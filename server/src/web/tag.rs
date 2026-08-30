use crate::api::error::{ApiError, ApiResult};
use crate::app::AppState;
use crate::config::{Action, RegexType};
use crate::extract::{Ctx, HxRequest, Json, MergeBody, Offset, Path, Query, ResourceParams};
use crate::model::enums::ResourceType;
use crate::resource::field::Mask;
use crate::resource::tag::{Field, TagInfo};
use crate::resource::tag_category::TagCategoryInfo;
use crate::schema::{tag, tag_name};
use crate::string::SmallString;
use crate::time::DateTime;
use crate::web::form::FormField;
use crate::web::form::tag::{DeletePathForm, EditPathForm, Focus, MergePathForm, Operation};
use crate::web::pager::{Page, Pager};
use crate::web::{Html, Message, Tab, WebError, WebResult};
use crate::{api, time, web};
use askama::Template;
use axum::response::{IntoResponse, Redirect, Response};
use axum::{Router, routing};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl};
use serde::{Deserialize, Serialize};
use server_macros::Deref;
use std::num::NonZeroU64;
use tokio::try_join;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/tags", routing::get(list))
        .route("/tag/{name}", routing::get(summary_tab))
        .route("/tag/{name}/edit", routing::get(edit_tab).post(edit_submit))
        .route("/tag/{name}/merge", routing::get(merge_tab).post(merge_submit))
        .route("/tag/{name}/delete", routing::get(delete_tab).post(delete_submit))
}

const LIMIT: NonZeroU64 = NonZeroU64::new(50).unwrap();
const DELETION_FLAG: &str = "tag-deleted";

const SUMMARY_FIELDS: [Field; 7] = [
    Field::Version,
    Field::Description,
    Field::Category,
    Field::Names,
    Field::Implications,
    Field::Suggestions,
    Field::Usages,
];
const MERGE_FIELDS: [Field; 2] = [Field::Version, Field::Names];
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

struct TagPage<T> {
    ctx: Ctx,
    active_tab: Tab,
    active_tag_tab: TagTab,
    tag: T,
    categories: Vec<TagCategoryInfo>,
    focus: Focus,
    message: Message,
}

impl TagPage<TagInfo> {
    async fn new(ctx: Ctx, path: Path<SmallString>, active_tag_tab: TagTab) -> WebResult<Self> {
        get_tag_and_categories(ctx.clone(), path, SUMMARY_FIELDS.into())
            .await
            .map(|(tag, categories)| Self {
                ctx,
                active_tab: Tab::Tag,
                active_tag_tab,
                tag,
                categories,
                focus: Focus::None,
                message: Message::None,
            })
            .map_err(WebError::from)
    }
}

#[derive(Deref, Template)]
#[template(path = "pages/tag/summary.html")]
struct SummaryPageTemplate(TagPage<TagInfo>);

#[derive(Template)]
#[template(path = "pages/tag/summary.html", block = "tag")]
struct SummaryFragmentTemplate {
    active_tag_tab: TagTab,
    tag: TagInfo,
}

async fn summary_tab(ctx: Ctx, path: Path<SmallString>, hx: HxRequest) -> WebResult<Html> {
    if hx.full_page() {
        let page_info = TagPage::new(ctx, path, TagTab::Summary).await?;
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

#[derive(Deref, Template)]
#[template(path = "pages/tag/edit.html")]
struct EditPageTemplate(TagPage<EditPathForm>);

#[derive(Deref, Template)]
#[template(path = "pages/tag/edit.html", block = "tag")]
struct EditFragmentTemplate(TagPage<EditPathForm>);

async fn edit_tab(ctx: Ctx, path: Path<SmallString>, hx: HxRequest) -> WebResult<Html> {
    let fields = edit_response_fields(&ctx);
    let (tag, categories) = get_tag_and_categories(ctx.clone(), path, fields).await?;
    let page_info = TagPage {
        ctx,
        active_tab: Tab::Tag,
        active_tag_tab: TagTab::Edit,
        tag: EditPathForm::initialize(tag)?,
        categories,
        focus: Focus::None,
        message: Message::None,
    };

    if hx.full_page() {
        EditPageTemplate(page_info).render()
    } else {
        EditFragmentTemplate(page_info).render()
    }
    .map(Html)
    .map_err(WebError::from)
}

async fn edit_submit(ctx: Ctx, hx: HxRequest, form: EditPathForm) -> WebResult<Html> {
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
            match api::tag::update(ctx.clone(), form.path(), Query(fields.into()), Json(form.to_body())).await {
                Ok(Json(tag)) => (EditPathForm::initialize(tag)?, focus, Message::Success),
                Err(err) => (form, focus, Message::Error(err)),
            }
        }
    };

    let categories = web::tag_category::get_categories(ctx.clone()).await?;
    let page_info = TagPage {
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
struct MergePageTemplate(TagPage<MergePathForm>);

#[derive(Template)]
#[template(path = "pages/tag/merge.html", block = "tag")]
struct MergeFragmentTemplate {
    ctx: Ctx,
    active_tag_tab: TagTab,
    tag: MergePathForm,
    message: Message,
}

async fn merge_tab(ctx: Ctx, path: Path<SmallString>, hx: HxRequest) -> WebResult<Html> {
    if hx.full_page() {
        let (tag, categories) = get_tag_and_categories(ctx.clone(), path, MERGE_FIELDS.into()).await?;
        let page_info = TagPage {
            ctx,
            active_tab: Tab::Tag,
            active_tag_tab: TagTab::Merge,
            tag: MergePathForm::initialize(&tag)?,
            categories,
            focus: Focus::None,
            message: Message::None,
        };
        MergePageTemplate(page_info).render()
    } else {
        let tag = get_tag(ctx.clone(), path, MERGE_FIELDS.into()).await?;
        MergeFragmentTemplate {
            ctx,
            active_tag_tab: TagTab::Merge,
            tag: MergePathForm::initialize(&tag)?,
            message: Message::None,
        }
        .render()
    }
    .map(Html)
    .map_err(WebError::from)
}

async fn merge_submit(ctx: Ctx, hx: HxRequest, form: MergePathForm) -> WebResult<Html> {
    let try_merge = {
        let remove = form.path.clone();
        let remove_version = form.version;
        let target_tag = form.target_tag.clone();
        async move |Ctx(ctx, connection_pool): Ctx| {
            let target_version: DateTime = connection_pool
                .transaction({
                    let target_name = target_tag.clone();
                    move |conn| {
                        tag::table
                            .inner_join(tag_name::table)
                            .select(tag::last_edit_time)
                            .filter(tag_name::name.eq(target_name))
                            .first(conn)
                            .optional()?
                            .ok_or(ApiError::NotFound(ResourceType::Tag))
                    }
                })
                .await?;

            let body = MergeBody {
                remove,
                merge_to: target_tag,
                remove_version,
                merge_to_version: target_version,
            };
            api::tag::merge(Ctx(ctx, connection_pool), Query(MERGE_FIELDS.into()), Json(body)).await
        }
    };

    let (form, message) = match try_merge(ctx.clone()).await {
        Ok(Json(tag)) => (MergePathForm::initialize(&tag)?, Message::Success),
        Err(err) => (form, Message::Error(err)),
    };
    if hx.full_page() {
        let categories = web::tag_category::get_categories(ctx.clone()).await?;
        let page_info = TagPage {
            ctx,
            active_tab: Tab::Tag,
            active_tag_tab: TagTab::Merge,
            tag: form,
            categories,
            focus: Focus::None,
            message,
        };
        MergePageTemplate(page_info).render()
    } else {
        MergeFragmentTemplate {
            ctx,
            active_tag_tab: TagTab::Merge,
            tag: form,
            message,
        }
        .render()
    }
    .map(Html)
    .map_err(WebError::from)
}

#[derive(Deref, Template)]
#[template(path = "pages/tag/delete.html")]
struct DeletePageTemplate(TagPage<DeletePathForm>);

#[derive(Template)]
#[template(path = "pages/tag/delete.html", block = "tag")]
struct DeleteFragmentTemplate {
    active_tag_tab: TagTab,
    tag: DeletePathForm,
    message: Message,
}

async fn delete_tab(ctx: Ctx, path: Path<SmallString>, hx: HxRequest) -> WebResult<Html> {
    if hx.full_page() {
        let (tag, categories) = get_tag_and_categories(ctx.clone(), path, DELETE_FIELDS.into()).await?;
        let page_info = TagPage {
            ctx,
            active_tab: Tab::Tag,
            active_tag_tab: TagTab::Delete,
            tag: DeletePathForm::initialize(&tag)?,
            categories,
            focus: Focus::None,
            message: Message::None,
        };
        DeletePageTemplate(page_info).render()
    } else {
        let tag = get_tag(ctx.clone(), path, DELETE_FIELDS.into()).await?;
        DeleteFragmentTemplate {
            active_tag_tab: TagTab::Delete,
            tag: DeletePathForm::initialize(&tag)?,
            message: Message::None,
        }
        .render()
    }
    .map(Html)
    .map_err(WebError::from)
}

async fn delete_submit(ctx: Ctx, hx: HxRequest, jar: CookieJar, form: DeletePathForm) -> WebResult<Response> {
    match api::tag::delete(ctx.clone(), form.path(), Json(form.to_body())).await {
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
                let page_info = TagPage {
                    ctx,
                    active_tab: Tab::Tag,
                    active_tag_tab: TagTab::Delete,
                    tag: form,
                    categories,
                    focus: Focus::None,
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
