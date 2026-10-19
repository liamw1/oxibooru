use crate::api::error::{ApiError, ApiResult};
use crate::api::post::{PostCreateBody, ReverseSearchBody};
use crate::app::AppState;
use crate::config::Action;
use crate::content::download;
use crate::content::upload::UploadToken;
use crate::extract::{Ctx, HxRequest, Json, JsonOrMultipart, Path, Query, ResourceParams};
use crate::model::enums::PostSafety;
use crate::resource::NotRequested;
use crate::resource::post::Field;
use crate::web::form::upload::{
    Content, Operation, PAUSE_ON_ERROR_DEFAULT, Preview, SimilarPost, UploadForm, Uploadable,
};
use crate::web::{Html, Message, Tab, WebError, WebResult};
use crate::{api, filesystem, web};
use askama::Template;
use axum::extract::{DefaultBodyLimit, Multipart};
use axum::response::{IntoResponse, Response};
use axum::{Router, routing};
use axum_extra::extract::CookieJar;
use std::collections::HashMap;
use strum::IntoEnumIterator;
use url::{Url, form_urlencoded};

pub fn routes(upload_limit: DefaultBodyLimit) -> Router<AppState> {
    let upload_capable_routes = Router::new()
        .route("/upload", routing::post(submit))
        .route_layer(upload_limit);
    Router::new()
        .route("/upload", routing::get(view))
        .merge(upload_capable_routes)
}

#[derive(PartialEq, Eq)]
enum QueueState {
    Idle,
    Uploading,
    Stopped,
}

#[derive(Template)]
#[template(path = "pages/upload.html")]
struct UploadTemplate {
    ctx: Ctx,
    active_tab: Tab,
    form: UploadForm,
    state: QueueState,
    messages: Vec<Message>,
}

async fn view(ctx: Ctx) -> WebResult<Html> {
    UploadTemplate {
        ctx,
        active_tab: Tab::Upload,
        form: UploadForm::initialize(),
        state: QueueState::Idle,
        messages: Vec::new(),
    }
    .render()
    .map(Html)
    .map_err(WebError::from)
}

type SavedFile = ApiResult<UploadToken>;

struct ParsedForm {
    form: UploadForm,
    new_files: Vec<(String, SavedFile)>,
    held_files: HashMap<String, SavedFile>,
}

async fn parse_form(ctx: &Ctx, mut multipart: Multipart) -> ApiResult<ParsedForm> {
    const CONTENT_FIELD: &str = "content";
    const HELD_FIELD_PREFIX: &str = "held[";

    let mut fields = Vec::new();
    let mut new_files = Vec::new();
    let mut held_files = HashMap::new();
    while let Some(field) = multipart.next_field().await? {
        let name = field.name().unwrap_or_default().to_owned();
        let held_key = name
            .strip_prefix(HELD_FIELD_PREFIX)
            .and_then(|rest| rest.strip_suffix(']'));
        if name != CONTENT_FIELD && held_key.is_none() {
            fields.push((name, field.text().await?));
            continue;
        }

        // Browsers send an empty part when no file is selected
        let file_name = field.file_name().unwrap_or_default().to_owned();
        if file_name.is_empty() {
            continue;
        }

        ctx.verify_privilege(Action::UploadCreate)?;
        let saved_file = filesystem::save_uploaded_file(&ctx.config, field).await;
        match held_key {
            Some(key) => {
                held_files.insert(key.to_owned(), saved_file);
            }
            None => new_files.push((file_name, saved_file)),
        }
    }

    // The regular fields are re-encoded so that they can be deserialized in the same way as any other form
    let encoded_fields = form_urlencoded::Serializer::new(String::new())
        .extend_pairs(fields)
        .finish();
    serde_qs::Config::new()
        .use_form_encoding(true)
        .deserialize_str(&encoded_fields)
        .map_err(|err| ApiError::FromStr(err.into()))
        .map(|form| ParsedForm {
            form,
            new_files,
            held_files,
        })
}

fn add_to_queue(ctx: &Ctx, form: &mut UploadForm, new_files: Vec<(String, SavedFile)>) {
    let anonymous = ctx.client.id.is_none();
    for (name, saved_file) in new_files {
        form.push_upload(Uploadable::new(name, saved_file.into(), anonymous));
    }

    // Keys end up in element IDs and field names, so only those made up of simple characters are accepted
    let is_valid_key = |key: &str| key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    for new_file in std::mem::take(&mut form.new_files) {
        if is_valid_key(&new_file.key) {
            form.push_upload(Uploadable::new(new_file.name, Content::Held(new_file.key), anonymous));
        }
    }

    for url in std::mem::take(&mut form.urls).split_whitespace() {
        let mut uploadable = Uploadable::new(url.to_owned(), Content::Url(url.to_owned()), anonymous);
        uploadable.source = Some(url.to_owned());
        form.push_upload(uploadable);
    }
}

async fn content_token(ctx: &Ctx, uploadable: &mut Uploadable) -> Result<UploadToken, String> {
    const MISSING_CONTENT_MESSAGE: &str = "This file is no longer available. Remove it and add it again.";
    let url = match &uploadable.content {
        Content::Token(token) => return Ok(token.clone()),
        Content::Error(err) => return Err(err.clone()),
        Content::Held(_) => return Err(MISSING_CONTENT_MESSAGE.to_owned()),
        Content::Url(url) => Url::parse(url).map_err(|err| err.to_string())?,
    };
    let token = download::from_url(ctx, url).await.map_err(|err| err.to_string())?;
    uploadable.content = Content::Token(token.clone());
    Ok(token)
}

enum Verdict {
    Proceed,
    Discard,
    Reject(String),
}

async fn check_for_similar(
    ctx: &Ctx,
    uploadable: &mut Uploadable,
    token: &UploadToken,
    form: &UploadForm,
) -> Result<Verdict, NotRequested> {
    const FIELDS: [Field; 2] = [Field::Id, Field::ThumbnailUrl];

    if uploadable.has_similar {
        return Ok(Verdict::Proceed);
    }

    let body = ReverseSearchBody {
        content_token: Some(token.clone()),
        content_url: None,
    };
    let search_result = api::post::reverse_search(ctx.clone(), Query(FIELDS.into()), JsonOrMultipart::Json(body)).await;
    let search_result = match search_result {
        Ok(Json(search_result)) => search_result,
        Err(err) => return Ok(Verdict::Reject(err.to_string())),
    };

    if let Some(exact_post) = search_result.exact_post {
        return Ok(if form.skip_duplicates {
            Verdict::Discard
        } else {
            Verdict::Reject(format!("Post already uploaded (@{})", exact_post.id()?))
        });
    }
    if search_result.similar_posts.is_empty() || form.upload_similar {
        return Ok(Verdict::Proceed);
    }
    uploadable.has_similar = true;

    let similar_count = search_result.similar_posts.len();
    uploadable.similar_posts = search_result
        .similar_posts
        .into_iter()
        .map(|similar| {
            Ok(SimilarPost {
                post_id: similar.post.id()?,
                thumbnail_url: similar.post.thumbnail_url()?.clone(),
                distance: similar.distance,
                copy_tags: false,
                add_relation: true,
            })
        })
        .collect::<Result<_, NotRequested>>()?;
    Ok(Verdict::Reject(format!(
        "Found {similar_count} similar posts. You can resume or discard this upload."
    )))
}

async fn create_post(ctx: &Ctx, uploadable: &Uploadable, token: UploadToken) -> WebResult<()> {
    let mut tags = Vec::new();
    let mut relations = Vec::new();
    for similar_post in &uploadable.similar_posts {
        if similar_post.copy_tags {
            let params = ResourceParams::from([Field::Tags]);
            let Json(post) = api::post::get(ctx.clone(), Path(similar_post.post_id), Query(params)).await?;
            for name in post.tags()?.iter().filter_map(|tag| tag.names.first()) {
                if !tags.contains(name) {
                    tags.push(name.clone());
                }
            }
        }
        if similar_post.add_relation {
            relations.push(similar_post.post_id);
        }
    }

    let body = PostCreateBody {
        safety: uploadable.safety,
        content_token: Some(token),
        content_url: None,
        thumbnail_token: None,
        thumbnail_url: None,
        source: uploadable.source.clone(),
        description: None,
        relations: (!relations.is_empty()).then_some(relations),
        anonymous: Some(uploadable.anonymous || ctx.client.id.is_none()),
        tags: (!tags.is_empty()).then_some(tags),
        notes: None,
        flags: None,
    };
    api::post::create(ctx.clone(), Query([Field::Id].into()), JsonOrMultipart::Json(body))
        .await
        .map(|_| ())
        .map_err(WebError::from)
}

async fn upload_one(ctx: &Ctx, form: &mut UploadForm, index: usize) -> WebResult<bool> {
    let Some(mut uploadable) = form.uploads.remove(&index) else {
        return Ok(true);
    };
    let result = match content_token(ctx, &mut uploadable).await {
        Ok(token) => match check_for_similar(ctx, &mut uploadable, &token, form).await? {
            Verdict::Proceed => match create_post(ctx, &uploadable, token).await {
                Err(WebError::Template(err)) => return Err(err.into()),
                result => result.map_err(|err| err.to_string()),
            },
            Verdict::Discard => Ok(()),
            Verdict::Reject(message) => Err(message),
        },
        Err(message) => Err(message),
    };
    let succeeded = result.is_ok();
    if let Err(message) = result {
        uploadable.message = Some(message);
        form.uploads.insert(index, uploadable);
    }
    Ok(succeeded)
}

async fn upload_queue(ctx: &Ctx, hx: &HxRequest, form: &mut UploadForm) -> WebResult<bool> {
    while let Some(index) = form.next_pending_upload() {
        let succeeded = upload_one(ctx, form, index).await?;
        if !succeeded && form.pause_on_error {
            return Ok(true);
        }
        if hx.htmx() {
            break;
        }
    }
    Ok(false)
}

fn posts_redirect(hx: &HxRequest, jar: CookieJar) -> Response {
    const POSTS_URL: &str = "/posts";

    // Uploads after the first are requested by an element that doesn't update the address bar,
    // so have HTMX navigate instead of following the redirect itself
    if hx.htmx() {
        (web::with_flash(jar), [("HX-Location", POSTS_URL)], "").into_response()
    } else {
        web::redirect(POSTS_URL, hx, jar)
    }
}

async fn submit(ctx: Ctx, hx: HxRequest, jar: CookieJar, multipart: Multipart) -> WebResult<Response> {
    let ParsedForm {
        mut form,
        new_files,
        mut held_files,
    } = parse_form(&ctx, multipart).await?;

    // The upload options are only rendered alongside a queue, so they keep their defaults until then
    if form.uploads.is_empty() {
        form.pause_on_error = PAUSE_ON_ERROR_DEFAULT;
    }

    for uploadable in form.uploads.values_mut() {
        if let Content::Held(key) = &uploadable.content
            && let Some(saved_file) = held_files.remove(key)
        {
            uploadable.content = saved_file.into();
        }
    }
    add_to_queue(&ctx, &mut form, new_files);

    let mut messages = Vec::new();
    let mut state = QueueState::Idle;
    let upload_count = form.uploads.len();
    match form.operation {
        Operation::Init => unreachable!(),
        Operation::Remove(index) => form.remove_upload(index),
        Operation::MoveUp(index) => form.move_upload_up(index),
        Operation::MoveDown(index) => form.move_upload_down(index),
        Operation::Add | Operation::Cancel => {}
        Operation::Upload | Operation::Continue => {
            if let Operation::Upload = form.operation {
                form.clear_upload_messages();
            }

            let starting_with_htmx = hx.htmx() && form.operation == Operation::Upload;
            let paused = !starting_with_htmx && upload_queue(&ctx, &hx, &mut form).await?;
            let has_pending = form.next_pending_upload().is_some();
            if form.uploads.is_empty() {
                if upload_count > 0 && messages.is_empty() {
                    return Ok(posts_redirect(&hx, jar));
                }
            } else if has_pending && !paused {
                state = QueueState::Uploading;
            } else {
                state = QueueState::Stopped;
                messages.push(Message::Attention);
            }
        }
    }

    UploadTemplate {
        ctx,
        active_tab: Tab::Upload,
        form,
        state,
        messages,
    }
    .render()
    .map(Html)
    .map(Html::into_response)
    .map_err(WebError::from)
}
