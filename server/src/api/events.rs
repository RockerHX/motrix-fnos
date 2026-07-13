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
mod tests;
