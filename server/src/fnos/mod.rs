use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::header::{AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE, HOST};
use hyper::{Method, Request};
use hyper_util::rt::TokioIo;
use serde::Deserialize;
use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::net::UnixStream;
use tokio::time::timeout;

pub const API_TOKEN_ENV: &str = "TRIM_API_TOKEN";
pub const GATEWAY_SOCKET_PATH: &str = "/var/run/trim_open_gateway_apiscope.socket";
const GATEWAY_HTTP_PATH: &str = "/api/v1/trimapp";
const SHARED_FOLDERS_REQUEST: &str = "trim.file.getSharedAccessibleFolders";
const APP_NAME: &str = "motrix";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_MAX_RESPONSE_BYTES: usize = 1024 * 1024;
static REQUEST_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedAccessibleFolders {
    pub paths: Vec<String>,
    pub http_status: u16,
    pub business_code: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FnosApiError {
    TokenMissing,
    TokenInvalid,
    SocketUnavailable,
    Timeout,
    Transport,
    ResponseTooLarge,
    Rejected {
        http_status: Option<u16>,
        business_code: Option<i64>,
    },
    InvalidResponse,
}

impl fmt::Display for FnosApiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TokenMissing => formatter.write_str("fnOS 未注入开放 API Token"),
            Self::TokenInvalid => formatter.write_str("fnOS 开放 API Token 格式无效"),
            Self::SocketUnavailable => formatter.write_str("fnOS 开放 API Socket 不可访问"),
            Self::Timeout => formatter.write_str("fnOS 开放 API 在 5 秒内未响应"),
            Self::Transport => formatter.write_str("调用 fnOS 开放 API 时发生传输错误"),
            Self::ResponseTooLarge => formatter.write_str("fnOS 开放 API 响应超过大小限制"),
            Self::Rejected {
                http_status,
                business_code,
            } => write!(
                formatter,
                "fnOS 开放 API 拒绝请求（HTTP {}，业务码 {}）",
                http_status
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "未知".to_string()),
                business_code
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "未知".to_string())
            ),
            Self::InvalidResponse => formatter.write_str("fnOS 开放 API 返回了无效响应"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FnosApiClient {
    socket_path: PathBuf,
    request_timeout: Duration,
    max_response_bytes: usize,
}

impl Default for FnosApiClient {
    fn default() -> Self {
        Self {
            socket_path: PathBuf::from(GATEWAY_SOCKET_PATH),
            request_timeout: DEFAULT_TIMEOUT,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        }
    }
}

impl FnosApiClient {
    #[cfg(test)]
    fn with_limits(
        socket_path: PathBuf,
        request_timeout: Duration,
        max_response_bytes: usize,
    ) -> Self {
        Self {
            socket_path,
            request_timeout,
            max_response_bytes,
        }
    }

    pub async fn query_shared_accessible_folders(
        &self,
    ) -> Result<SharedAccessibleFolders, FnosApiError> {
        let token = std::env::var(API_TOKEN_ENV).ok();
        self.query_shared_accessible_folders_with_token(token.as_deref())
            .await
    }

    async fn query_shared_accessible_folders_with_token(
        &self,
        token: Option<&str>,
    ) -> Result<SharedAccessibleFolders, FnosApiError> {
        let token = validate_token(token)?;
        timeout(self.request_timeout, self.query(token))
            .await
            .map_err(|_| FnosApiError::Timeout)?
    }

    async fn query(&self, token: &str) -> Result<SharedAccessibleFolders, FnosApiError> {
        let stream = UnixStream::connect(&self.socket_path)
            .await
            .map_err(classify_socket_error)?;
        let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
            .await
            .map_err(|_| FnosApiError::Transport)?;
        tokio::spawn(async move {
            let _ = connection.await;
        });

        let body = serde_json::to_vec(&serde_json::json!({
            "reqId": format!(
                "motrix-{}-{}",
                std::process::id(),
                REQUEST_ID.fetch_add(1, Ordering::Relaxed)
            ),
            "req": SHARED_FOLDERS_REQUEST,
            "appName": APP_NAME,
            "data": {}
        }))
        .map_err(|_| FnosApiError::InvalidResponse)?;
        let request = Request::builder()
            .method(Method::POST)
            .uri(GATEWAY_HTTP_PATH)
            .header(HOST, "localhost")
            .header(CONTENT_TYPE, "application/json")
            .header(CONTENT_LENGTH, body.len())
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .body(Full::new(Bytes::from(body)))
            .map_err(|_| FnosApiError::TokenInvalid)?;
        let response = sender
            .send_request(request)
            .await
            .map_err(|_| FnosApiError::Transport)?;
        let http_status = response.status().as_u16();
        let response_body =
            read_limited_body(response.into_body(), self.max_response_bytes).await?;
        let envelope = serde_json::from_slice::<GatewayEnvelope>(&response_body)
            .map_err(|_| FnosApiError::InvalidResponse)?;

        if !(200..=299).contains(&http_status) || envelope.code != 0 {
            return Err(FnosApiError::Rejected {
                http_status: Some(http_status),
                business_code: Some(envelope.code),
            });
        }
        let paths = envelope
            .data
            .and_then(|data| data.paths)
            .ok_or(FnosApiError::InvalidResponse)?;

        Ok(SharedAccessibleFolders {
            paths,
            http_status,
            business_code: envelope.code,
        })
    }
}

#[derive(Debug, Deserialize)]
struct GatewayEnvelope {
    code: i64,
    #[serde(rename = "msg")]
    _message: String,
    data: Option<GatewayData>,
}

#[derive(Debug, Deserialize)]
struct GatewayData {
    paths: Option<Vec<String>>,
}

fn validate_token(token: Option<&str>) -> Result<&str, FnosApiError> {
    let token = token.ok_or(FnosApiError::TokenMissing)?;
    if token.is_empty() || token.trim() != token || token.contains(['\r', '\n']) {
        return Err(FnosApiError::TokenInvalid);
    }
    Ok(token)
}

fn classify_socket_error(error: std::io::Error) -> FnosApiError {
    match error.kind() {
        std::io::ErrorKind::NotFound
        | std::io::ErrorKind::PermissionDenied
        | std::io::ErrorKind::ConnectionRefused => FnosApiError::SocketUnavailable,
        _ => FnosApiError::Transport,
    }
}

async fn read_limited_body(
    mut body: hyper::body::Incoming,
    max_bytes: usize,
) -> Result<Vec<u8>, FnosApiError> {
    let mut output = Vec::new();
    while let Some(frame) = body.frame().await {
        let frame = frame.map_err(|_| FnosApiError::Transport)?;
        if let Some(data) = frame.data_ref() {
            if output.len().saturating_add(data.len()) > max_bytes {
                return Err(FnosApiError::ResponseTooLarge);
            }
            output.extend_from_slice(data);
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests;
