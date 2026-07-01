mod app;
mod aria2;
pub mod error;

use crate::app::HttpAppState;
use axum::Router;
use std::sync::Arc;

#[cfg(test)]
use crate::app::{bootstrap_http_app_state, ServerRuntimeConfig, DEFAULT_HTTP_ADDR};
#[cfg(test)]
use axum::body::{to_bytes, Body};
#[cfg(test)]
use axum::http::{Request, StatusCode};
#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
use tower::ServiceExt;

pub fn router(state: Arc<HttpAppState>) -> Router {
    Router::new()
        .nest("/api", app::routes())
        .nest("/api", aria2::routes())
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::app::{AppInfo, BackendPing};
    use crate::api::error::ErrorResponse;
    use crate::aria2::{Aria2ConfigStatus, Aria2RpcStatus};
    use crate::runtime::Aria2ProcessStatus;
    use serde::de::DeserializeOwned;
    use std::sync::atomic::Ordering;

    #[tokio::test]
    async fn app_routes_return_expected_payloads() {
        let state = test_state(None).await;
        let app = router(state);

        let info = response_json::<AppInfo>(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/app/info")
                        .body(Body::empty())
                        .expect("request should build"),
                )
                .await
                .expect("response should succeed"),
            StatusCode::OK,
        )
        .await;
        assert_eq!(info.name, "Motrix FNOS");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.backend_status, "ready");

        let ping = response_json::<BackendPing>(
            app.oneshot(
                Request::builder()
                    .uri("/api/app/ping")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed"),
            StatusCode::OK,
        )
        .await;
        assert!(ping.ok);
        assert_eq!(ping.message, "Rust 后端通信正常");
    }

    #[tokio::test]
    async fn aria2_routes_return_status_payloads() {
        let explicit_path = temp_dir("aria2-config").join("aria2-next");
        std::fs::create_dir_all(explicit_path.parent().expect("parent should exist"))
            .expect("dir should create");
        std::fs::write(&explicit_path, b"").expect("binary should exist");

        let state = test_state(Some(explicit_path.display().to_string())).await;
        let app = router(state);

        let config = response_json::<Aria2ConfigStatus>(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/aria2/config")
                        .body(Body::empty())
                        .expect("request should build"),
                )
                .await
                .expect("response should succeed"),
            StatusCode::OK,
        )
        .await;
        assert!(config.configured);
        assert!(config.path_exists);
        assert_eq!(config.path.as_deref(), Some(explicit_path.to_string_lossy().as_ref()));

        let process = response_json::<Aria2ProcessStatus>(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/aria2/process")
                        .body(Body::empty())
                        .expect("request should build"),
                )
                .await
                .expect("response should succeed"),
            StatusCode::OK,
        )
        .await;
        assert!(!process.running);
        assert_eq!(process.message, "Aria2 进程未启动");

        let rpc = response_json::<Aria2RpcStatus>(
            app.oneshot(
                Request::builder()
                    .uri("/api/aria2/rpc")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed"),
            StatusCode::OK,
        )
        .await;
        assert!(!rpc.connected);
        assert!(rpc.version.is_none());
    }

    #[tokio::test]
    async fn aria2_mutation_routes_reject_when_runtime_is_exiting() {
        let state = test_state(None).await;
        state.core.is_exiting.store(true, Ordering::SeqCst);
        let app = router(state);

        for uri in ["/api/aria2/start", "/api/aria2/stop"] {
            let error = response_json::<ErrorResponse>(
                app.clone()
                    .oneshot(
                        Request::builder()
                            .method("POST")
                            .uri(uri)
                            .body(Body::empty())
                            .expect("request should build"),
                    )
                    .await
                    .expect("response should succeed"),
                StatusCode::CONFLICT,
            )
            .await;
            assert_eq!(error.code, "runtime_exiting");
            assert_eq!(error.message, "服务正在退出，不能执行当前操作");
        }
    }

    async fn test_state(aria2_path: Option<String>) -> Arc<HttpAppState> {
        let app_data_dir = temp_dir("api-state");
        let runtime = ServerRuntimeConfig {
            database_path: app_data_dir.join("motrix-fnos.sqlite"),
            app_data_dir,
            http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
            aria2_path: aria2_path.map(PathBuf::from),
        };

        bootstrap_http_app_state(&runtime)
            .await
            .expect("state should bootstrap")
    }

    async fn response_json<T: DeserializeOwned>(
        response: axum::response::Response,
        expected_status: StatusCode,
    ) -> T {
        assert_eq!(response.status(), expected_status);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        serde_json::from_slice(&body).expect("response json should deserialize")
    }

    fn temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "motrix-fnos-{}-{}",
            label,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be valid")
                .as_nanos()
        ))
    }
}
