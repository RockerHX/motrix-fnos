use crate::app::{HttpAppState, RuntimeEvent};
use crate::runtime::visible_tasks_snapshot;
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

pub fn routes() -> Router<Arc<HttpAppState>> {
    Router::new().route("/events", get(stream_events))
}

async fn stream_events(State(state): State<Arc<HttpAppState>>) -> impl IntoResponse {
    let mut receiver = state.runtime_events.subscribe();
    let initial_event = RuntimeEvent::TasksSnapshot(crate::app::TasksSnapshotPayload {
        tasks: visible_tasks_snapshot(&state).unwrap_or_default(),
    });
    let stream = async_stream::stream! {
        if let Some(event) = runtime_event_to_sse(initial_event) {
            yield Ok::<Event, Infallible>(event);
        }

        loop {
            match receiver.recv().await {
                Ok(event) => {
                    if let Some(event) = runtime_event_to_sse(event) {
                        yield Ok::<Event, Infallible>(event);
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    state.core.debug_logs.warn(
                        "runtime.events",
                        format!("SSE 事件流检测到丢帧，已跳过 {} 条事件", skipped),
                    );
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keepalive"),
    )
}

fn runtime_event_to_sse(event: RuntimeEvent) -> Option<Event> {
    match event {
        RuntimeEvent::TasksSnapshot(payload) => serde_json::to_string(&payload)
            .ok()
            .map(|payload| Event::default().event("tasks.snapshot").data(payload)),
        RuntimeEvent::RuntimeExiting(payload) => serde_json::to_string(&payload)
            .ok()
            .map(|payload| Event::default().event("runtime.exiting").data(payload)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{bootstrap_http_app_state, ServerRuntimeConfig, DEFAULT_HTTP_ADDR};
    use crate::tasks::{DownloadTask, DownloadTaskStatus};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn sse_route_sends_initial_tasks_snapshot_event() {
        let app_data_dir = temp_dir("events-state");
        let runtime = ServerRuntimeConfig {
            database_path: app_data_dir.join("motrix-fnos.sqlite"),
            app_data_dir,
            http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
            aria2_path: None,
        };
        let state = bootstrap_http_app_state(&runtime)
            .await
            .expect("state should bootstrap");
        state
            .core
            .download_tasks
            .lock()
            .expect("tasks should lock")
            .push(sample_task());
        let app = Router::new().nest("/api", routes()).with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/events")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        let mut body = response.into_body();
        let frame = body
            .frame()
            .await
            .expect("first frame should exist")
            .expect("first frame should be ok");
        let bytes = frame.into_data().expect("frame should contain data");
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("event: tasks.snapshot"));
        assert!(text.contains("\"archive.zip\""));
    }

    fn sample_task() -> DownloadTask {
        DownloadTask {
            id: 1,
            url: "https://example.com/archive.zip".to_string(),
            file_name: "archive.zip".to_string(),
            save_dir: temp_dir("events-downloads").display().to_string(),
            gid: Some("gid-1".to_string()),
            status: DownloadTaskStatus::Active,
            total_length: 1024,
            completed_length: 256,
            download_speed: 128,
            error_code: None,
            error_message: None,
            file_path: None,
            created_at: 1,
            updated_at: 2,
        }
    }

    fn temp_dir(label: &str) -> std::path::PathBuf {
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
