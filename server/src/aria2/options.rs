use super::rpc::Aria2RpcClient;
use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use crate::settings::service::{
    normalize_min_split_size, MAX_CONCURRENT_DOWNLOADS_LIMIT, MAX_CONNECTION_PER_SERVER_LIMIT,
    MAX_CONNECT_TIMEOUT_LIMIT, MAX_SPLIT_LIMIT, MAX_TRIES_LIMIT,
};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aria2GlobalOptions {
    pub max_concurrent_downloads: u32,
    pub max_connection_per_server: u32,
    pub split: u32,
    pub min_split_size: String,
    pub connect_timeout: u32,
    pub max_tries: u32,
    pub download_limit: u64,
    pub upload_limit: u64,
}

pub async fn apply_global_options(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    options: &Aria2GlobalOptions,
    debug_logs: Option<&DebugLogStore>,
) -> Result<(), String> {
    let request_body = build_change_global_option_request(config, options);
    client
        .request::<serde_json::Value>(config, &request_body)
        .await
        .and_then(|response| response.into_result())
        .map_err(|error| format!("应用 Aria2 下载配置失败：{}", error))?;

    if let Some(debug_logs) = debug_logs {
        debug_logs.info(
            "aria2.options",
            format!(
                "已应用 Aria2 下载配置：最大并发 {}，每服务器连接 {}，分片 {}，最小分片 {}，连接超时 {} 秒，重试 {} 次，下载限速 {} B/s，上传限速 {} B/s",
                options.max_concurrent_downloads,
                options.max_connection_per_server,
                options.split,
                options.min_split_size,
                options.connect_timeout,
                options.max_tries,
                options.download_limit,
                options.upload_limit
            ),
        );
    }

    Ok(())
}

pub async fn apply_bt_proxy(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    proxy_url: Option<&str>,
) -> Result<(), String> {
    if config.engine_generation() != crate::config::aria2::EngineGeneration::NativeV2 {
        return if proxy_url.is_some() {
            Err("BT 总代理需要使用已验证的 NativeV2 引擎".to_string())
        } else {
            Ok(())
        };
    }
    let target = proxy_url.unwrap_or_default();
    let mut params = Vec::new();
    if !config.rpc_secret.is_empty() {
        params.push(Value::String(format!("token:{}", config.rpc_secret)));
    }
    params.push(serde_json::json!({ "bt-proxy": target }));
    let change = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "motrix-fnos-bt-proxy-apply",
        "method": "aria2.changeGlobalOption",
        "params": params,
    });
    client
        .request::<Value>(config, &change)
        .await
        .and_then(|response| response.into_result())
        .map_err(|error| format!("应用 BT 全局代理失败：{error}"))?;

    let mut params = Vec::new();
    if !config.rpc_secret.is_empty() {
        params.push(Value::String(format!("token:{}", config.rpc_secret)));
    }
    let verify = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "motrix-fnos-bt-proxy-verify",
        "method": "aria2.getGlobalOption",
        "params": params,
    });
    let options = client
        .request::<Value>(config, &verify)
        .await
        .and_then(|response| response.into_result())
        .map_err(|error| format!("核验 BT 全局代理失败：{error}"))?;
    if options.get("bt-proxy").and_then(Value::as_str) != Some(target) {
        return Err("Aria2 BT 全局代理核验失败".to_string());
    }
    Ok(())
}

// Keep the explicit mapping from persisted settings to normalized Aria2 options local to this boundary.
#[allow(clippy::too_many_arguments)]
pub fn global_options_from_values(
    max_concurrent_downloads: u32,
    download_limit: u64,
    upload_limit: u64,
    max_connection_per_server: u32,
    split: u32,
    min_split_size: &str,
    connect_timeout: u32,
    max_tries: u32,
) -> Aria2GlobalOptions {
    Aria2GlobalOptions {
        max_concurrent_downloads: max_concurrent_downloads.clamp(1, MAX_CONCURRENT_DOWNLOADS_LIMIT),
        max_connection_per_server: max_connection_per_server
            .clamp(1, MAX_CONNECTION_PER_SERVER_LIMIT),
        split: split.clamp(1, MAX_SPLIT_LIMIT),
        min_split_size: normalize_min_split_size(min_split_size),
        connect_timeout: connect_timeout.clamp(1, MAX_CONNECT_TIMEOUT_LIMIT),
        max_tries: max_tries.clamp(1, MAX_TRIES_LIMIT),
        download_limit,
        upload_limit,
    }
}

fn build_change_global_option_request(
    config: &Aria2Config,
    options: &Aria2GlobalOptions,
) -> serde_json::Value {
    let mut params = Vec::new();
    if !config.rpc_secret.is_empty() {
        params.push(serde_json::json!(format!("token:{}", config.rpc_secret)));
    }

    params.push(serde_json::json!({
        "max-concurrent-downloads": options.max_concurrent_downloads.to_string(),
        "max-connection-per-server": options.max_connection_per_server.to_string(),
        "split": options.split.to_string(),
        "min-split-size": options.min_split_size,
        "connect-timeout": options.connect_timeout.to_string(),
        "max-tries": options.max_tries.to_string(),
        "max-overall-download-limit": options.download_limit.to_string(),
        "max-overall-upload-limit": options.upload_limit.to_string(),
    }));

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": "motrix-fnos-change-global-option",
        "method": "aria2.changeGlobalOption",
        "params": params,
    })
}

#[cfg(test)]
mod tests;
