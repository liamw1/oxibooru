use crate::api::error::{ApiError, ApiResult};
use crate::app::AppState;
use crate::config::{Action, RegexType};
use crate::extract::{Ctx, HxRequest, Json, MergeBody, Offset, Path, Query, ResourceParams};
use crate::model::enums::ResourceType;
use crate::resource::field::Mask;
use crate::resource::pool::{Field, PoolInfo};
use crate::resource::pool_category::PoolCategoryInfo;
use crate::schema::{pool, pool_name};
use crate::string::SmallString;
use crate::time::DateTime;
use crate::web::form::pool::{DeletePathForm, EditPathForm, MergePathForm};
use crate::web::pager::{Page, Pager};
use crate::web::{Html, Message, Tab, WebError, WebResult};
use crate::{api, time, web};
use askama::Template;
use axum::response::{IntoResponse, Response};
use axum::{Router, routing};
use axum_extra::extract::CookieJar;
use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl};
use serde::{Deserialize, Serialize};
use server_macros::Deref;
use std::num::NonZeroU64;
use tokio::try_join;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/pools", routing::get(list))
        .route("/pool/{name}", routing::get(summary_tab))
        .route("/pool/{name}/edit", routing::get(edit_tab).post(edit_submit))
        .route("/pool/{name}/merge", routing::get(merge_tab).post(merge_submit))
        .route("/pool/{name}/delete", routing::get(delete_tab).post(delete_submit))
}

const LIMIT: NonZeroU64 = NonZeroU64::new(50).unwrap();

const SUMMARY_FIELDS: [Field; 5] = [
    Field::Id,
    Field::Description,
    Field::Category,
    Field::Names,
    Field::PostCount,
];
const MERGE_FIELDS: [Field; 2] = [Field::Version, Field::Names];
const DELETE_FIELDS: [Field; 3] = [Field::Version, Field::Names, Field::PostCount];

fn edit_response_fields(ctx: &Ctx) -> Mask<Field> {
    let mut fields = [Field::Version, Field::Names].into();
    if ctx.has_privilege(Action::PoolEditCategory) {
        fields |= Field::Category;
    }
    if ctx.has_privilege(Action::PoolEditDescription) {
        fields |= Field::Description;
    }
    if ctx.has_privilege(Action::PoolEditPost) {
        fields |= Field::Posts
    }
    fields
}

async fn get_id(Ctx(_, connection_pool): &Ctx, Path(name): Path<SmallString>) -> ApiResult<i64> {
    connection_pool
        .transaction(|conn| {
            pool_name::table
                .select(pool_name::pool_id)
                .filter(pool_name::name.eq(name))
                .first(conn)
                .optional()?
                .ok_or(ApiError::NotFound(ResourceType::Pool))
        })
        .await
}

async fn get_pool(ctx: Ctx, path: Path<SmallString>, fields: Mask<Field>) -> ApiResult<PoolInfo> {
    let pool_id = get_id(&ctx, path).await?;
    api::pool::get(ctx, Path(pool_id), Query(fields.into()))
        .await
        .map(|Json(tag)| tag)
}

async fn get_pool_and_categories(
    ctx: Ctx,
    path: Path<SmallString>,
    fields: Mask<Field>,
) -> ApiResult<(PoolInfo, Vec<PoolCategoryInfo>)> {
    let tag_future = get_pool(ctx.clone(), path, fields);
    let categories_future = web::pool_category::get_categories(ctx.clone());
    try_join!(tag_future, categories_future)
}

async fn fetch_target_version(ctx: &Ctx, target_pool: SmallString) -> ApiResult<DateTime> {
    let Ctx(_, connection_pool) = ctx;
    connection_pool
        .transaction(|conn| {
            pool::table
                .inner_join(pool_name::table)
                .select(pool::last_edit_time)
                .filter(pool_name::name.eq(target_pool))
                .first(conn)
                .optional()?
                .ok_or(ApiError::NotFound(ResourceType::Tag))
        })
        .await
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
#[template(path = "pages/pool/list.html")]
struct ListTemplate<'a> {
    ctx: Ctx,
    active_tab: Tab,
    pools: Vec<PoolInfo>,
    categories: Vec<PoolCategoryInfo>,
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
        Field::Id,
        Field::CreationTime,
        Field::Category,
        Field::Names,
        Field::PostCount,
    ]
    .into();

    let query = params.search_text.clone();
    let resource_params = Query(ResourceParams { query, fields });
    let page_params = Query(offset.to_page_params(LIMIT));
    let Json(response) = api::pool::list(ctx.clone(), resource_params, page_params).await?;
    let categories = web::pool_category::get_categories(ctx.clone()).await?;

    let params = params.simplify();
    let pager = Pager::build("pools", &params, page_params, response.total);

    let (jar, message) = web::redirect_message(jar);
    ListTemplate {
        ctx,
        active_tab: Tab::Pool,
        pools: response.results,
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
enum PoolTab {
    Summary,
    Edit,
    Merge,
    Delete,
}

struct PoolPage<T> {
    ctx: Ctx,
    active_tab: Tab,
    active_pool_tab: PoolTab,
    pool: T,
    categories: Vec<PoolCategoryInfo>,
    message: Message,
}

impl PoolPage<PoolInfo> {
    async fn new(ctx: Ctx, path: Path<SmallString>, active_pool_tab: PoolTab) -> WebResult<Self> {
        get_pool_and_categories(ctx.clone(), path, SUMMARY_FIELDS.into())
            .await
            .map(|(pool, categories)| Self {
                ctx,
                active_tab: Tab::Pool,
                active_pool_tab,
                pool,
                categories,
                message: Message::None,
            })
            .map_err(WebError::from)
    }
}

#[derive(Deref, Template)]
#[template(path = "pages/pool/summary.html")]
struct SummaryPageTemplate(PoolPage<PoolInfo>);

#[derive(Template)]
#[template(path = "pages/pool/summary.html", block = "pool")]
struct SummaryFragmentTemplate {
    active_pool_tab: PoolTab,
    pool: PoolInfo,
}

async fn summary_tab(ctx: Ctx, path: Path<SmallString>, hx: HxRequest) -> WebResult<Html> {
    if hx.full_page() {
        let page_info = PoolPage::new(ctx, path, PoolTab::Summary).await?;
        SummaryPageTemplate(page_info).render()
    } else {
        let pool = get_pool(ctx.clone(), path, SUMMARY_FIELDS.into()).await?;
        SummaryFragmentTemplate {
            active_pool_tab: PoolTab::Summary,
            pool,
        }
        .render()
    }
    .map(Html)
    .map_err(WebError::from)
}

#[derive(Deref, Template)]
#[template(path = "pages/pool/edit.html")]
struct EditPageTemplate(PoolPage<EditPathForm>);

#[derive(Deref, Template)]
#[template(path = "pages/pool/edit.html", block = "pool")]
struct EditFragmentTemplate(PoolPage<EditPathForm>);

async fn edit_tab(ctx: Ctx, path: Path<SmallString>, hx: HxRequest, jar: CookieJar) -> WebResult<Response> {
    let fields = edit_response_fields(&ctx);
    let (pool, categories) = get_pool_and_categories(ctx.clone(), path, fields).await?;

    let (jar, message) = web::redirect_message(jar);
    let page_info = PoolPage {
        ctx,
        active_tab: Tab::Pool,
        active_pool_tab: PoolTab::Edit,
        pool: EditPathForm::initialize(pool)?,
        categories,
        message,
    };

    if hx.full_page() {
        EditPageTemplate(page_info).render()
    } else {
        EditFragmentTemplate(page_info).render()
    }
    .map(|html| (jar, Html(html)).into_response())
    .map_err(WebError::from)
}

async fn edit_submit(ctx: Ctx, hx: HxRequest, jar: CookieJar, form: EditPathForm) -> WebResult<Response> {
    let Ok(primary_name) = form.primary_name();
    let fields = edit_response_fields(&ctx);
    let update_pool = {
        let ctx = ctx.clone();
        let path = form.path();
        let body = form.to_body().map(Json);
        async move || {
            let body = body?;
            let path = get_id(&ctx, path).await.map(Path)?;
            api::pool::update(ctx.clone(), path, Query(fields.into()), body).await
        }
    };

    let (updated_form, message) = match update_pool().await {
        Ok(Json(pool)) => {
            let new_primary_name = pool.primary_name()?;
            if !hx.htmx() || new_primary_name != primary_name {
                let new_url = format!("/pool/{new_primary_name}/edit");
                return Ok(web::redirect(&new_url, &hx, jar));
            }
            (EditPathForm::initialize(pool)?, Message::Success)
        }
        Err(err) => (form, Message::Error(err)),
    };

    let categories = web::pool_category::get_categories(ctx.clone()).await?;
    let page_info = PoolPage {
        ctx,
        active_tab: Tab::Pool,
        active_pool_tab: PoolTab::Edit,
        pool: updated_form,
        categories,
        message,
    };
    if hx.full_page() {
        EditPageTemplate(page_info).render()
    } else {
        EditFragmentTemplate(page_info).render()
    }
    .map(Html)
    .map(Html::into_response)
    .map_err(WebError::from)
}

#[derive(Deref, Template)]
#[template(path = "pages/pool/merge.html")]
struct MergePageTemplate(PoolPage<MergePathForm>);

#[derive(Template)]
#[template(path = "pages/pool/merge.html", block = "pool")]
struct MergeFragmentTemplate {
    ctx: Ctx,
    active_pool_tab: PoolTab,
    pool: MergePathForm,
    message: Message,
}

async fn merge_tab(ctx: Ctx, path: Path<SmallString>, hx: HxRequest, jar: CookieJar) -> WebResult<Response> {
    let (jar, message) = web::redirect_message(jar);
    if hx.full_page() {
        let (pool, categories) = get_pool_and_categories(ctx.clone(), path, MERGE_FIELDS.into()).await?;
        let page_info = PoolPage {
            ctx,
            active_tab: Tab::Pool,
            active_pool_tab: PoolTab::Merge,
            pool: MergePathForm::initialize(&pool)?,
            categories,
            message,
        };
        MergePageTemplate(page_info).render()
    } else {
        let pool = get_pool(ctx.clone(), path, MERGE_FIELDS.into()).await?;
        MergeFragmentTemplate {
            ctx,
            active_pool_tab: PoolTab::Merge,
            pool: MergePathForm::initialize(&pool)?,
            message,
        }
        .render()
    }
    .map(|html| (jar, Html(html)).into_response())
    .map_err(WebError::from)
}

async fn merge_submit(ctx: Ctx, hx: HxRequest, jar: CookieJar, form: MergePathForm) -> WebResult<Response> {
    let merge_result = async {
        let remove = get_id(&ctx, form.path()).await?;
        let merge_to = get_id(&ctx, Path(form.target_pool.clone())).await?;
        let merge_to_version = fetch_target_version(&ctx, form.target_pool.clone()).await?;
        let body = MergeBody {
            remove,
            merge_to,
            remove_version: form.version,
            merge_to_version,
        };
        api::pool::merge(ctx.clone(), Query(MERGE_FIELDS.into()), Json(body)).await
    }
    .await;

    let (form, message) = match merge_result {
        Ok(Json(tag)) => {
            if !hx.htmx() {
                let Ok(primary_name) = form.primary_name();
                let url = format!("/pool/{primary_name}/merge");
                return Ok(web::redirect(&url, &hx, jar));
            }
            (MergePathForm::initialize(&tag)?, Message::Success)
        }
        Err(err) => (form, Message::Error(err)),
    };
    if hx.full_page() {
        let categories = web::pool_category::get_categories(ctx.clone()).await?;
        let page_info = PoolPage {
            ctx,
            active_tab: Tab::Pool,
            active_pool_tab: PoolTab::Merge,
            pool: form,
            categories,
            message,
        };
        MergePageTemplate(page_info).render()
    } else {
        MergeFragmentTemplate {
            ctx,
            active_pool_tab: PoolTab::Merge,
            pool: form,
            message,
        }
        .render()
    }
    .map(Html)
    .map(Html::into_response)
    .map_err(WebError::from)
}

#[derive(Deref, Template)]
#[template(path = "pages/pool/delete.html")]
struct DeletePageTemplate(PoolPage<DeletePathForm>);

#[derive(Template)]
#[template(path = "pages/pool/delete.html", block = "pool")]
struct DeleteFragmentTemplate {
    active_pool_tab: PoolTab,
    pool: DeletePathForm,
    message: Message,
}

async fn delete_tab(ctx: Ctx, path: Path<SmallString>, hx: HxRequest) -> WebResult<Html> {
    if hx.full_page() {
        let (pool, categories) = get_pool_and_categories(ctx.clone(), path, DELETE_FIELDS.into()).await?;
        let page_info = PoolPage {
            ctx,
            active_tab: Tab::Pool,
            active_pool_tab: PoolTab::Delete,
            pool: DeletePathForm::initialize(&pool)?,
            categories,
            message: Message::None,
        };
        DeletePageTemplate(page_info).render()
    } else {
        let pool = get_pool(ctx.clone(), path, DELETE_FIELDS.into()).await?;
        DeleteFragmentTemplate {
            active_pool_tab: PoolTab::Delete,
            pool: DeletePathForm::initialize(&pool)?,
            message: Message::None,
        }
        .render()
    }
    .map(Html)
    .map_err(WebError::from)
}

async fn delete_submit(ctx: Ctx, hx: HxRequest, jar: CookieJar, form: DeletePathForm) -> WebResult<Response> {
    match api::tag::delete(ctx.clone(), form.path(), Json(form.to_body())).await {
        Ok(Json(())) => Ok(web::redirect("/pools", &hx, jar)),
        Err(err) => {
            let message = Message::Error(err);
            if hx.full_page() {
                let categories = web::pool_category::get_categories(ctx.clone()).await?;
                let page_info = PoolPage {
                    ctx,
                    active_tab: Tab::Tag,
                    active_pool_tab: PoolTab::Delete,
                    pool: form,
                    categories,
                    message,
                };
                DeletePageTemplate(page_info).render()
            } else {
                DeleteFragmentTemplate {
                    active_pool_tab: PoolTab::Delete,
                    pool: form,
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
