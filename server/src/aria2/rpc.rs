use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use crate::runtime::Aria2LifecycleCoordinator;
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

const ARIA2_RPC_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const ARIA2_RPC_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone)]
pub struct Aria2RpcClient {
    client: reqwest::Client,
    lifecycle: Option<std::sync::Arc<Aria2LifecycleCoordinator>>,
}

impl Aria2RpcClient {
    pub fn new() -> Self {
        Self::with_timeouts(ARIA2_RPC_CONNECT_TIMEOUT, ARIA2_RPC_REQUEST_TIMEOUT)
    }

    pub(crate) fn with_timeouts(connect_timeout: Duration, request_timeout: Duration) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(connect_timeout)
            .timeout(request_timeout)
            .build()
            .expect("Aria2 RPC HTTP client should build");
        Self {
            client,
            lifecycle: None,
        }
    }

    pub(crate) fn with_lifecycle(lifecycle: std::sync::Arc<Aria2LifecycleCoordinator>) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(ARIA2_RPC_CONNECT_TIMEOUT)
            .timeout(ARIA2_RPC_REQUEST_TIMEOUT)
            .build()
            .expect("Aria2 RPC HTTP client should build");
        Self {
            client,
            lifecycle: Some(lifecycle),
        }
    }

    pub(crate) async fn request<T>(
        &self,
        config: &Aria2Config,
        request_body: &serde_json::Value,
    ) -> Result<Aria2RpcResponse<T>, Aria2RpcError>
    where
        T: DeserializeOwned,
    {
        let _request_lease = self
            .lifecycle
            .as_ref()
            .map(|lifecycle| lifecycle.acquire_request())
            .transpose()
            .map_err(Aria2RpcError::Lifecycle)?;
        let response = self
            .client
            .post(config.rpc_url())
            .json(request_body)
            .send()
            .await
            .map_err(classify_transport_error)?;
        let status = response.status();
        if !status.is_success() {
            return Err(Aria2RpcError::HttpStatus(status));
        }

        response
            .json::<Aria2RpcResponse<T>>()
            .await
            .map_err(|error| Aria2RpcError::InvalidResponse(error.to_string()))
    }
}

impl Default for Aria2RpcClient {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct Aria2RpcResponse<T> {
    pub(crate) result: Option<T>,
    pub(crate) error: Option<Aria2RpcServerError>,
}

impl<T> Aria2RpcResponse<T> {
    pub(crate) fn into_result(self) -> Result<T, Aria2RpcError> {
        if let Some(error) = self.error {
            return Err(Aria2RpcError::Remote(error));
        }
        self.result.ok_or(Aria2RpcError::MissingResult)
    }

    pub(crate) fn into_optional_result(self) -> Result<Option<T>, Aria2RpcError> {
        if let Some(error) = self.error {
            return Err(Aria2RpcError::Remote(error));
        }
        Ok(self.result)
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct Aria2RpcServerError {
    pub(crate) code: Option<i64>,
    pub(crate) message: String,
}

#[derive(Debug)]
pub(crate) enum Aria2RpcError {
    Lifecycle(String),
    ConnectionFailed(String),
    OutcomeUnknown(String),
    HttpStatus(StatusCode),
    InvalidResponse(String),
    Remote(Aria2RpcServerError),
    MissingResult,
}

impl fmt::Display for Aria2RpcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lifecycle(error) => {
                write!(formatter, "Aria2 生命周期请求被拒绝（可重试）：{error}")
            }
            Self::ConnectionFailed(error) => write!(formatter, "Aria2 RPC 连接失败：{error}"),
            Self::OutcomeUnknown(error) => write!(
                formatter,
                "Aria2 RPC 结果未知：请求可能已送达，请等待对账确认（{error}）"
            ),
            Self::HttpStatus(status) => write!(formatter, "Aria2 RPC 返回 HTTP 状态 {status}"),
            Self::InvalidResponse(error) => write!(formatter, "Aria2 RPC 响应解析失败：{error}"),
            Self::Remote(error) => match error.code {
                Some(code) => write!(
                    formatter,
                    "Aria2 RPC 返回错误（代码 {code}）：{}",
                    error.message
                ),
                None => write!(formatter, "Aria2 RPC 返回错误：{}", error.message),
            },
            Self::MissingResult => write!(formatter, "Aria2 RPC 响应缺少结果"),
        }
    }
}

impl std::error::Error for Aria2RpcError {}

fn classify_transport_error(error: reqwest::Error) -> Aria2RpcError {
    if error.is_connect() {
        return Aria2RpcError::ConnectionFailed(error.to_string());
    }

    Aria2RpcError::OutcomeUnknown(error.to_string())
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Aria2RpcStatus {
    pub connected: bool,
    pub version: Option<String>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Aria2VersionResult {
    version: String,
}

pub async fn ping_rpc(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    debug_logs: Option<&DebugLogStore>,
) -> Aria2RpcStatus {
    let mut params = Vec::new();
    if !config.rpc_secret.is_empty() {
        params.push(format!("token:{}", config.rpc_secret));
    }

    let request_body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "motrix-fnos-version-check",
        "method": "aria2.getVersion",
        "params": params,
    });

    let version = match client
        .request::<Aria2VersionResult>(config, &request_body)
        .await
        .and_then(Aria2RpcResponse::into_result)
    {
        Ok(version) => version,
        Err(Aria2RpcError::MissingResult) => {
            if let Some(debug_logs) = debug_logs {
                debug_logs.error("aria2.rpc", "Aria2 RPC 响应缺少版本信息");
            }
            return Aria2RpcStatus {
                connected: false,
                version: None,
                message: "Aria2 RPC 响应缺少版本信息".to_string(),
            };
        }
        Err(error) => {
            if let Some(debug_logs) = debug_logs {
                debug_logs.warn("aria2.rpc", format!("Aria2 RPC 暂不可用：{}", error));
            }
            return Aria2RpcStatus {
                connected: false,
                version: None,
                message: error.to_string(),
            };
        }
    };
    if let Some(debug_logs) = debug_logs {
        debug_logs.info(
            "aria2.rpc",
            format!("Aria2 RPC ready，版本 {}", version.version),
        );
    }
    Aria2RpcStatus {
        connected: true,
        version: Some(version.version.clone()),
        message: format!("Aria2 RPC 连接正常，版本 {}", version.version),
    }
}

pub(crate) async fn change_global_log_level(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    level: crate::aria2::Aria2LogLevel,
) -> Result<(), Aria2RpcError> {
    let mut params = Vec::new();
    if !config.rpc_secret.is_empty() {
        params.push(serde_json::json!(format!("token:{}", config.rpc_secret)));
    }
    params.push(serde_json::json!({
        "log-level": level.as_aria2_option(),
    }));

    let request_body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "motrix-fnos-change-log-level",
        "method": "aria2.changeGlobalOption",
        "params": params,
    });
    client
        .request::<serde_json::Value>(config, &request_body)
        .await?
        .into_result()
        .map(|_| ())
}

#[cfg(test)]
mod tests;
