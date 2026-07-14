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
use axum::http::header::{CACHE_CONTROL, EXPIRES, PRAGMA};
use axum::http::{HeaderValue, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::any;
use axum::Router;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};

pub fn management_router(state: Arc<HttpAppState>) -> Router {
    management_router_with_static_dir(state, static_assets_dir())
}

pub fn jsonrpc_router(state: Arc<HttpAppState>) -> Router {
    Router::new().merge(jsonrpc::routes()).with_state(state)
}

fn management_router_with_static_dir(state: Arc<HttpAppState>, static_dir: PathBuf) -> Router {
    let index_file = static_dir.join("index.html");
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
    let api_routes = Router::new()
        .merge(auth::public_routes())
        .merge(session_auth_routes)
        .merge(admin_auth_routes)
        .merge(management_routes)
        .merge(event_routes)
        .fallback(api_not_found);

    Router::new()
        .nest("/api", api_routes)
        .route("/jsonrpc", any(api_not_found))
        .route("/jsonrpc/", any(api_not_found))
        .route("/jsonrpc/*path", any(api_not_found))
        .fallback_service(ServeDir::new(static_dir).not_found_service(ServeFile::new(index_file)))
        .layer(middleware::from_fn(no_cache_headers))
        .with_state(state)
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
