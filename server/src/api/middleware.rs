use crate::api::ApiResult;
use crate::auth::{Client as AuthClient, header};
use crate::model::enums::UserRank;
use crate::model::snapshot::Snapshot;
use crate::schema::snapshot;
use crate::{api, config, db, update};
use axum::extract::Request;
use axum::http::Method;
use axum::http::header::AUTHORIZATION;
use axum::middleware::Next;
use axum::response::Response;
use diesel::{
    ExpressionMethods, OptionalExtension, PgConnection, QueryDsl, QueryResult, RunQueryDsl, SelectableHelper,
};
use reqwest::Client;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue, REFERER};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use tracing::warn;
use url::Url;

/// Attempts to authorizes user by either username/password or user token.
pub async fn auth(mut request: Request, next: Next) -> ApiResult<Response> {
    let auth_header = request.headers().get(AUTHORIZATION);
    let client = if let Some(auth_value) = auth_header {
        let auth_str = auth_value.to_str()?;
        header::authenticate_user(auth_str)
    } else {
        Ok(AuthClient::new(None, UserRank::Anonymous))
    }?;

    // If client is not anonymous and query contains "bump-login", update login time
    if let Some(user_id) = client.id
        && let Some(query) = request.uri().query()
        && query.contains("bump-login")
    {
        update::user::last_login_time(user_id)?;
    }

    request.extensions_mut().insert(client);
    Ok(next.run(request).await)
}

/// Sends snapshot data to webhook URLs after modifying requests.
pub async fn post_to_webhooks(request: Request, next: Next) -> ApiResult<Response> {
    let can_modify_database = matches!(request.method(), &Method::POST | &Method::PUT | &Method::DELETE);
    let response = next.run(request).await;

    if can_modify_database {
        let mut conn = db::get_connection()?;
        let last_posted_snapshot = LAST_POSTED_SNAPSHOT.load(Ordering::SeqCst);
        let new_snapshots = snapshot::table
            .select(Snapshot::as_select())
            .filter(snapshot::id.gt(last_posted_snapshot))
            .order_by(snapshot::id)
            .load(&mut conn)?;

        for snapshot in new_snapshots {
            post_snapshot(snapshot);
        }
    }

    Ok(response)
}

/// Initializes last posted snapshot counter using highest ID snapshot
pub fn initialize_snapshot_counter(conn: &mut PgConnection) -> QueryResult<()> {
    let latest_snapshot_id = snapshot::table
        .select(snapshot::id)
        .order_by(snapshot::id.desc())
        .first(conn)
        .optional()
        .map(Option::unwrap_or_default)?;
    LAST_POSTED_SNAPSHOT.store(latest_snapshot_id, Ordering::SeqCst);
    Ok(())
}

static LAST_POSTED_SNAPSHOT: AtomicI64 = AtomicI64::new(i64::MAX);

/// Sends `snapshot` data to webhooks if it hasn't already been posted by another thread.
fn post_snapshot(snapshot: Snapshot) {
    loop {
        let last_posted_snapshot = LAST_POSTED_SNAPSHOT.load(Ordering::SeqCst);
        if snapshot.id <= last_posted_snapshot {
            return;
        }

        if LAST_POSTED_SNAPSHOT
            .compare_exchange(last_posted_snapshot, snapshot.id, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            break;
        }
    }

    let snapshot = Arc::new(snapshot);
    for url in &config::get().webhooks {
        tokio::spawn(post_to_webhook(url.clone(), snapshot.clone()));
    }
}

/// Sends `snapshot` data to given `url`.
async fn post_to_webhook(url: Url, snapshot: Arc<Snapshot>) {
    const APPLICATION_JSON: HeaderValue = HeaderValue::from_static("application/json");
    let post = async || {
        let mut headers = HeaderMap::new();
        headers.insert(REFERER, HeaderValue::from_str(url.as_str())?);
        headers.insert(CONTENT_TYPE, APPLICATION_JSON);

        let client = Client::builder().default_headers(headers).build()?;
        let response = client.post(url.clone()).json(&snapshot).send().await?;
        response.error_for_status().map(|_| ()).map_err(api::Error::from)
    };

    if let Err(err) = post().await {
        warn!("Could not post snapshot to {url}. Details:\n{err}");
    }
}
