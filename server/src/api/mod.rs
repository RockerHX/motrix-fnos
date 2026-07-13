mod app;
mod aria2;
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
use axum::http::header::{CACHE_CONTROL, EXPIRES, PRAGMA};
use axum::http::{HeaderValue, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::Router;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};

pub fn router(state: Arc<HttpAppState>) -> Router {
    let static_dir = static_assets_dir();
    let index_file = static_dir.join("index.html");

    Router::new()
        .nest("/api", app::routes())
        .nest("/api", aria2::routes())
        .nest("/api", settings::routes())
        .nest("/api", storage::routes())
        .nest("/api", debug_logs::routes())
        .nest("/api", tasks::routes())
        .nest("/api", events::routes())
        .merge(jsonrpc::routes())
        .fallback_service(ServeDir::new(static_dir).not_found_service(ServeFile::new(index_file)))
        .layer(middleware::from_fn(no_cache_headers))
        .with_state(state)
}

pub fn gateway_router(state: Arc<HttpAppState>) -> Router {
    Router::new()
        .nest("/app/motrix", router(state))
        .layer(middleware::from_fn(require_gateway_admin))
}

pub fn jsonrpc_router(state: Arc<HttpAppState>) -> Router {
    jsonrpc::routes().with_state(state)
}

async fn require_gateway_admin(request: Request<Body>, next: Next) -> Response {
    let headers = request.headers();
    let user_id = headers
        .get("x-trim-userid")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if user_id.is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({
                "code": "gateway_auth_required",
                "message": "请通过飞牛 fnOS 登录后访问 Motrix",
            })),
        )
            .into_response();
    }

    let is_admin = headers
        .get("x-trim-isadmin")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if !is_admin {
        return (
            StatusCode::FORBIDDEN,
            axum::Json(serde_json::json!({
                "code": "admin_required",
                "message": "Motrix 仅允许管理员访问",
            })),
        )
            .into_response();
    }

    next.run(request).await
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
