mod app;
mod aria2;
mod auth;
mod debug_logs;
pub mod error;
mod events;
mod extract;
mod jsonrpc;
mod settings;
mod storage;
mod tasks;

use crate::app::HttpAppState;
use axum::body::Body;
use axum::extract::DefaultBodyLimit;
use axum::http::header::{CACHE_CONTROL, EXPIRES, PRAGMA};
use axum::http::{HeaderValue, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::Router;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tower::limit::ConcurrencyLimitLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::services::ServeDir;
use tower_http::timeout::TimeoutLayer;

const API_BODY_LIMIT: usize = 1024 * 1024;
const TORRENT_UPLOAD_BODY_LIMIT: usize = 12 * 1024 * 1024;
const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MANAGEMENT_HTTP_CONCURRENCY_LIMIT: usize = 64;
const TORRENT_UPLOAD_CONCURRENCY_LIMIT: usize = 8;
pub(super) const JSONRPC_HTTP_CONCURRENCY_LIMIT: usize = 32;

#[derive(Clone, Copy)]
pub(super) struct HttpResourceLimits {
    pub(super) body_limit: usize,
    pub(super) concurrency_limit: usize,
    pub(super) timeout: Duration,
}

const MANAGEMENT_HTTP_LIMITS: HttpResourceLimits = HttpResourceLimits {
    body_limit: API_BODY_LIMIT,
    concurrency_limit: MANAGEMENT_HTTP_CONCURRENCY_LIMIT,
    timeout: HTTP_REQUEST_TIMEOUT,
};

const TORRENT_UPLOAD_LIMITS: HttpResourceLimits = HttpResourceLimits {
    body_limit: TORRENT_UPLOAD_BODY_LIMIT,
    concurrency_limit: TORRENT_UPLOAD_CONCURRENCY_LIMIT,
    timeout: HTTP_REQUEST_TIMEOUT,
};

pub(super) const JSONRPC_HTTP_LIMITS: HttpResourceLimits = HttpResourceLimits {
    body_limit: API_BODY_LIMIT,
    concurrency_limit: JSONRPC_HTTP_CONCURRENCY_LIMIT,
    timeout: HTTP_REQUEST_TIMEOUT,
};

pub fn management_router(state: Arc<HttpAppState>) -> Router {
    management_router_with_static_dir(state, static_assets_dir())
}

pub fn jsonrpc_router(state: Arc<HttpAppState>) -> Router {
    Router::new().merge(jsonrpc::routes()).with_state(state)
}

fn management_router_with_static_dir(state: Arc<HttpAppState>, static_dir: PathBuf) -> Router {
    let management_routes = Router::new()
        .merge(app::routes())
        .merge(aria2::routes())
        .merge(settings::routes())
        .merge(storage::routes())
        .merge(debug_logs::routes())
        .merge(tasks::routes())
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::management_auth,
        ));
    let torrent_upload_routes = with_http_resource_limits(
        tasks::torrent_routes().route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::management_auth,
        )),
        TORRENT_UPLOAD_LIMITS,
    );
    let event_routes = events::routes().route_layer(middleware::from_fn_with_state(
        state.clone(),
        auth::event_auth,
    ));
    let session_auth_routes = auth::session_routes().route_layer(middleware::from_fn_with_state(
        state.clone(),
        auth::session_auth,
    ));
    let admin_auth_routes = auth::admin_routes().route_layer(middleware::from_fn_with_state(
        state.clone(),
        auth::admin_auth,
    ));
    let api_routes = with_http_resource_limits(
        Router::new()
            .merge(auth::public_routes())
            .merge(app::readiness_routes())
            .merge(session_auth_routes)
            .merge(admin_auth_routes)
            .merge(management_routes),
        MANAGEMENT_HTTP_LIMITS,
    )
    .merge(torrent_upload_routes)
    .merge(event_routes)
    .fallback(api_not_found);

    Router::new()
        .nest("/api", api_routes)
        .fallback_service(ServeDir::new(static_dir))
        .layer(middleware::from_fn(no_cache_headers))
        .with_state(state)
}

pub(super) fn with_http_resource_limits<S>(
    router: Router<S>,
    limits: HttpResourceLimits,
) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    router
        .layer(DefaultBodyLimit::max(limits.body_limit))
        .layer(RequestBodyLimitLayer::new(limits.body_limit))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            limits.timeout,
        ))
        .layer(ConcurrencyLimitLayer::new(limits.concurrency_limit))
}

async fn api_not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}

async fn no_cache_headers(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate, max-age=0"),
    );
    headers.insert(PRAGMA, HeaderValue::from_static("no-cache"));
    headers.insert(EXPIRES, HeaderValue::from_static("0"));
    response
}

fn static_assets_dir() -> PathBuf {
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(app_dir) = current_exe.parent().and_then(|bin_dir| bin_dir.parent()) {
            let packaged = app_dir.join("ui").join("dist");
            if packaged.join("index.html").is_file() {
                return packaged;
            }
        }
    }

    if let Some(repo_root) = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent() {
        let staged = repo_root
            .join("packaging")
            .join("fnos")
            .join("app")
            .join("ui")
            .join("dist");
        if staged.join("index.html").is_file() {
            return staged;
        }

        let dev_dist = repo_root.join("dist");
        if dev_dist.join("index.html").is_file() {
            return dev_dist;
        }
    }

    PathBuf::from("dist")
}

#[cfg(test)]
mod tests;
