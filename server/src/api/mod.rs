mod app;
mod aria2;
mod debug_logs;
pub mod error;
mod extract;
mod settings;
mod tasks;

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
        .nest("/api", settings::routes())
        .nest("/api", debug_logs::routes())
        .nest("/api", tasks::routes())
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::app::{AppInfo, BackendPing};
    use crate::api::error::ErrorResponse;
    use crate::aria2::{Aria2ConfigStatus, Aria2RpcStatus};
    use crate::debug_logs::DebugLogEntry;
    use crate::settings::service::{AppConfig, UiPreferences};
    use crate::runtime::Aria2ProcessStatus;
    use serde::de::DeserializeOwned;
    use std::collections::BTreeMap;
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

    #[tokio::test]
    async fn settings_routes_round_trip_payloads_and_log_rpc_warning() {
        let state = test_state(None).await;
        let app = router(state.clone());

        let default_settings = response_json::<AppConfig>(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/settings")
                        .body(Body::empty())
                        .expect("request should build"),
                )
                .await
                .expect("response should succeed"),
            StatusCode::OK,
        )
        .await;
        assert!(default_settings.default_download_dir.ends_with("Downloads"));

        let updated_settings = response_json::<AppConfig>(
            app.clone()
                .oneshot(json_request(
                    "PUT",
                    "/api/settings",
                    &AppConfig {
                        default_download_dir: "/tmp/custom".to_string(),
                        max_concurrent_downloads: 0,
                        download_limit: 1024,
                        upload_limit: 2048,
                        auto_start_enabled: true,
                        notifications_enabled: true,
                    },
                ))
                .await
                .expect("response should succeed"),
            StatusCode::OK,
        )
        .await;
        assert_eq!(updated_settings.default_download_dir, "/tmp/custom");
        assert_eq!(updated_settings.max_concurrent_downloads, 1);
        assert_eq!(updated_settings.download_limit, 1024);
        assert_eq!(updated_settings.upload_limit, 2048);
        assert!(updated_settings.auto_start_enabled);
        assert!(updated_settings.notifications_enabled);

        let stored_settings = response_json::<AppConfig>(
            app.oneshot(
                Request::builder()
                    .uri("/api/settings")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed"),
            StatusCode::OK,
        )
        .await;
        assert_eq!(stored_settings, updated_settings);
        assert!(state.core.debug_logs.list().iter().any(|entry| {
            entry.module == "settings" && entry.message.contains("下载配置将在下次启动后生效")
        }));
    }

    #[tokio::test]
    async fn ui_preferences_routes_round_trip_payloads() {
        let state = test_state(None).await;
        let app = router(state);

        let default_preferences = response_json::<UiPreferences>(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/ui-preferences")
                        .body(Body::empty())
                        .expect("request should build"),
                )
                .await
                .expect("response should succeed"),
            StatusCode::OK,
        )
        .await;
        assert!(default_preferences.task_table_column_widths.is_empty());

        let mut widths = BTreeMap::new();
        widths.insert("name".to_string(), 280);
        let payload = UiPreferences {
            task_table_column_widths: widths.clone(),
        };
        let updated_preferences = response_json::<UiPreferences>(
            app.clone()
                .oneshot(json_request("PUT", "/api/ui-preferences", &payload))
                .await
                .expect("response should succeed"),
            StatusCode::OK,
        )
        .await;
        assert_eq!(updated_preferences.task_table_column_widths, widths);

        let stored_preferences = response_json::<UiPreferences>(
            app.oneshot(
                Request::builder()
                    .uri("/api/ui-preferences")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed"),
            StatusCode::OK,
        )
        .await;
        assert_eq!(stored_preferences, updated_preferences);
    }

    #[tokio::test]
    async fn debug_log_routes_list_and_clear_entries() {
        let state = test_state(None).await;
        state.core.debug_logs.info("test", "first");
        state.core.debug_logs.warn("test", "second");
        let app = router(state.clone());

        let logs = response_json::<Vec<DebugLogEntry>>(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/debug-logs")
                        .body(Body::empty())
                        .expect("request should build"),
                )
                .await
                .expect("response should succeed"),
            StatusCode::OK,
        )
        .await;
        assert!(logs.iter().any(|entry| entry.message == "first"));
        assert!(logs.iter().any(|entry| entry.message == "second"));

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/debug-logs")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed");
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(state.core.debug_logs.list().is_empty());
    }

    #[tokio::test]
    async fn invalid_json_payload_uses_unified_error_response() {
        let state = test_state(None).await;
        let app = router(state);

        let error = response_json::<ErrorResponse>(
            app.oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/settings")
                    .header("content-type", "application/json")
                    .body(Body::from("{"))
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed"),
            StatusCode::BAD_REQUEST,
        )
        .await;
        assert_eq!(error.code, "invalid_json");
        assert!(error.message.contains("请求体 JSON 无效"));
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

    fn json_request<T: serde::Serialize>(method: &str, uri: &str, payload: &T) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(payload).expect("payload should serialize"),
            ))
            .expect("request should build")
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
