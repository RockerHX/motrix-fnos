use crate::api::auth::{event_context_is_authorized, EventAuthContext};
use crate::app::{HttpAppState, RuntimeEvent};
use crate::runtime::current_tasks_snapshot;
use axum::extract::{Extension, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

const AUTH_REVALIDATION_INTERVAL: Duration = Duration::from_secs(15);

pub fn routes() -> Router<Arc<HttpAppState>> {
    Router::new().route("/events", get(stream_events))
}

async fn stream_events(
    State(state): State<Arc<HttpAppState>>,
    Extension(event_auth_context): Extension<EventAuthContext>,
) -> impl IntoResponse {
    let mut receiver = state.runtime_events.subscribe();
    let initial_event =
        current_tasks_snapshot(&state).unwrap_or_else(|_| crate::app::TasksSnapshotPayload {
            revision: 0,
            tasks: Vec::new(),
        });
    let stream = async_stream::stream! {
        if !event_context_is_authorized(&state, &event_auth_context).await {
            return;
        }
        if let Some(event) = runtime_event_to_sse(RuntimeEvent::TasksSnapshot(initial_event)) {
            yield Ok::<Event, Infallible>(event);
        }

        let mut auth_revalidation = tokio::time::interval(AUTH_REVALIDATION_INTERVAL);
        auth_revalidation.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        auth_revalidation.tick().await;
        loop {
            tokio::select! {
                _ = auth_revalidation.tick() => {
                    if !event_context_is_authorized(&state, &event_auth_context).await {
                        break;
                    }
                }
                received = receiver.recv() => {
                    if !event_context_is_authorized(&state, &event_auth_context).await {
                        break;
                    }
                    match received {
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
                            if let Ok(snapshot) = current_tasks_snapshot(&state) {
                                if let Some(event) = runtime_event_to_sse(RuntimeEvent::TasksSnapshot(snapshot)) {
                                    yield Ok::<Event, Infallible>(event);
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
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
