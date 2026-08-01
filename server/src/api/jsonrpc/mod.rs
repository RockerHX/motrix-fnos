mod add_uri;
mod auth;
mod methods;
mod types;

use crate::app::HttpAppState;
use axum::body::{Body, Bytes};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Extension, State};
use axum::http::header::{CONTENT_TYPE, SEC_WEBSOCKET_PROTOCOL};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Router;
use serde_json::Value;
use std::sync::Arc;
use types::rpc_error;

const JSONRPC_WEBSOCKET_MESSAGE_LIMIT: usize = 256 * 1024;
const JSONRPC_WEBSOCKET_WRITE_BUFFER_SIZE: usize = 128 * 1024;
const JSONRPC_WEBSOCKET_MAX_WRITE_BUFFER_SIZE: usize =
    JSONRPC_WEBSOCKET_MESSAGE_LIMIT + JSONRPC_WEBSOCKET_WRITE_BUFFER_SIZE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JsonRpcAccess {
    Proxy,
    Lan,
}

pub fn routes(access: JsonRpcAccess) -> Router<Arc<HttpAppState>> {
    let http_routes = super::with_http_resource_limits(
        Router::new().route("/jsonrpc", post(handle_http_jsonrpc)),
        super::JSONRPC_HTTP_LIMITS,
    );
    let websocket_routes = Router::new().route(
        "/jsonrpc",
        axum::routing::get(handle_ws_jsonrpc).options(handle_jsonrpc_options),
    );

    http_routes.merge(websocket_routes).layer(Extension(access))
}

async fn handle_http_jsonrpc(
    State(state): State<Arc<HttpAppState>>,
    Extension(access): Extension<JsonRpcAccess>,
    body: Bytes,
) -> Response {
    let payload = match serde_json::from_slice::<Value>(&body) {
        Ok(payload) => methods::handle_jsonrpc_payload_with_access(&state, access, payload).await,
        Err(_) => rpc_error(Value::Null, -32700, "Parse error"),
    };
    jsonrpc_http_response(StatusCode::OK, payload)
}

async fn handle_jsonrpc_options() -> Response {
    let mut response = StatusCode::NO_CONTENT.into_response();
    insert_cors_headers(response.headers_mut());
    response
}

async fn handle_ws_jsonrpc(
    State(state): State<Arc<HttpAppState>>,
    Extension(access): Extension<JsonRpcAccess>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.protocols(["jsonrpc"])
        .max_frame_size(JSONRPC_WEBSOCKET_MESSAGE_LIMIT)
        .max_message_size(JSONRPC_WEBSOCKET_MESSAGE_LIMIT)
        .write_buffer_size(JSONRPC_WEBSOCKET_WRITE_BUFFER_SIZE)
        .max_write_buffer_size(JSONRPC_WEBSOCKET_MAX_WRITE_BUFFER_SIZE)
        .on_upgrade(move |socket| handle_jsonrpc_socket(socket, state, access))
}

async fn handle_jsonrpc_socket(
    mut socket: WebSocket,
    state: Arc<HttpAppState>,
    access: JsonRpcAccess,
) {
    while let Some(message) = socket.recv().await {
        let Ok(message) = message else {
            break;
        };

        let payload = match message {
            Message::Text(text) => serde_json::from_str::<Value>(&text),
            Message::Binary(bytes) => serde_json::from_slice::<Value>(&bytes),
            Message::Ping(bytes) => {
                let _ = socket.send(Message::Pong(bytes)).await;
                continue;
            }
            Message::Pong(_) => continue,
            Message::Close(_) => break,
        };

        let response = match payload {
            Ok(payload) => {
                methods::handle_jsonrpc_payload_with_access(&state, access, payload).await
            }
            Err(_) => rpc_error(Value::Null, -32700, "Parse error"),
        };

        if socket
            .send(Message::Text(response.to_string()))
            .await
            .is_err()
        {
            break;
        }
    }
}

fn jsonrpc_http_response(status: StatusCode, payload: Value) -> Response {
    let mut response = Response::builder()
        .status(status)
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
        .body(Body::from(payload.to_string()))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
    insert_cors_headers(response.headers_mut());
    response
}

fn insert_cors_headers(headers: &mut axum::http::HeaderMap) {
    headers.insert(
        HeaderName::from_static("access-control-allow-origin"),
        HeaderValue::from_static("*"),
    );
    headers.insert(
        HeaderName::from_static("access-control-allow-methods"),
        HeaderValue::from_static("GET, POST, OPTIONS"),
    );
    headers.insert(
        HeaderName::from_static("access-control-allow-headers"),
        HeaderValue::from_static("content-type, authorization"),
    );
    headers.insert(
        HeaderName::from_static("access-control-expose-headers"),
        HeaderValue::from_static(SEC_WEBSOCKET_PROTOCOL.as_str()),
    );
    headers.insert(
        HeaderName::from_static("access-control-allow-private-network"),
        HeaderValue::from_static("true"),
    );
}

#[cfg(test)]
mod tests;
