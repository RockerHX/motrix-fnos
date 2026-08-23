mod app;
mod aria2;
mod auth;
mod debug_logs;
mod diagnostics;
pub mod error;
mod events;
mod extract;
mod jsonrpc;
mod settings;
mod storage;
mod tasks;

use crate::app::HttpAppState;
use crate::tasks::repository::SqliteTaskRepository;
use crate::tasks::service::{RuntimeGuard, TaskService, TaskServiceDependencies};
use axum::body::Body;
use axum::extract::{ConnectInfo, DefaultBodyLimit, State};
use axum::http::header::{CACHE_CONTROL, EXPIRES, PRAGMA};
use axum::http::{HeaderValue, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::Router;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
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
const REQUEST_ID_HEADER: &str = "x-request-id";
static REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub(super) struct RequestContext {
    pub(super) request_id: String,
    pub(super) path: String,
}

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
    Router::new()
        .merge(jsonrpc::routes(jsonrpc::JsonRpcAccess::Proxy))
        .layer(middleware::from_fn(request_context))
        .with_state(state)
}

pub fn lan_jsonrpc_router(state: Arc<HttpAppState>) -> Router {
    Router::new()
        .merge(jsonrpc::routes(jsonrpc::JsonRpcAccess::Lan))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            authorize_lan_jsonrpc_peer,
        ))
        .layer(middleware::from_fn(request_context))
        .with_state(state)
}

pub(crate) fn build_task_service(state: &HttpAppState) -> TaskService<'_> {
    TaskService::new(TaskServiceDependencies {
        repository: Box::new(SqliteTaskRepository::new(&state.core.database.pool)),
        download_tasks: &state.core.download_tasks,
        next_task_id: &state.core.next_task_id,
        app_data_dir: &state.core.app_data_dir,
        debug_logs: &state.core.debug_logs,
        aria2_rpc: &state.aria2_rpc,
        aria2_lifecycle: &state.aria2_lifecycle,
        proxy_update_lock: &state.download_proxy_update_lock,
        runtime_guard: RuntimeGuard::new(&state.core.shutdown),
    })
}

async fn authorize_lan_jsonrpc_peer(
    State(state): State<Arc<HttpAppState>>,
    ConnectInfo(peer): ConnectInfo<std::net::SocketAddr>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if !state.lan_json_rpc_config().await.enabled {
        return StatusCode::NOT_FOUND.into_response();
    }
    if !is_rfc1918_peer(peer.ip()) {
        return StatusCode::FORBIDDEN.into_response();
    }
    next.run(request).await
}

fn is_rfc1918_peer(ip: std::net::IpAddr) -> bool {
    let std::net::IpAddr::V4(ip) = ip else {
        return false;
    };
    let octets = ip.octets();
    octets[0] == 10
        || (octets[0] == 172 && (16..=31).contains(&octets[1]))
        || (octets[0] == 192 && octets[1] == 168)
}

fn management_router_with_static_dir(state: Arc<HttpAppState>, static_dir: PathBuf) -> Router {
    let management_routes = Router::new()
        .merge(app::routes())
        .merge(aria2::routes())
        .merge(settings::routes())
        .merge(storage::routes())
        .merge(debug_logs::routes())
        .merge(diagnostics::routes())
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
        .layer(middleware::from_fn(request_context))
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

async fn request_context(mut request: Request<Body>, next: Next) -> Response {
    let request_id = new_request_id();
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    request.extensions_mut().insert(RequestContext {
        request_id: request_id.clone(),
        path: path.clone(),
    });
    let span = tracing::info_span!(
        "http_request",
        request_id = %request_id,
        method = %method,
        path = %path,
    );
    let mut response = tracing::Instrument::instrument(next.run(request), span).await;
    let header_value = HeaderValue::from_str(&request_id)
        .expect("server-generated request ID should be a valid header value");
    response
        .headers_mut()
        .insert(REQUEST_ID_HEADER, header_value);
    response
}

fn new_request_id() -> String {
    let sequence = REQUEST_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("req-{timestamp:x}-{sequence:x}")
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
