use super::rpc::EmptyJsonRpcResponse;
use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aria2GlobalOptions {
    pub max_concurrent_downloads: u32,
    pub download_limit: u64,
    pub upload_limit: u64,
}

pub async fn apply_global_options(
    config: &Aria2Config,
    options: &Aria2GlobalOptions,
    debug_logs: Option<&DebugLogStore>,
) -> Result<(), String> {
    let request_body = build_change_global_option_request(config, options);
    let response = reqwest::Client::new()
        .post(config.rpc_url())
        .json(&request_body)
        .send()
        .await
        .map_err(|error| format!("应用 Aria2 下载配置失败：无法连接 RPC（{}）", error))?;

    let rpc_response = response
        .json::<EmptyJsonRpcResponse>()
        .await
        .map_err(|error| format!("应用 Aria2 下载配置失败：响应解析失败（{}）", error))?;

    if let Some(error) = rpc_response.error {
        return Err(format!("应用 Aria2 下载配置失败：{}", error.message));
    }

    if let Some(debug_logs) = debug_logs {
        debug_logs.info(
            "aria2.options",
            format!(
                "已应用 Aria2 下载配置：最大并发 {}，下载限速 {} B/s，上传限速 {} B/s",
                options.max_concurrent_downloads, options.download_limit, options.upload_limit
            ),
        );
    }

    Ok(())
}

pub fn global_options_from_values(
    max_concurrent_downloads: u32,
    download_limit: u64,
    upload_limit: u64,
) -> Aria2GlobalOptions {
    Aria2GlobalOptions {
        max_concurrent_downloads: max_concurrent_downloads.clamp(1, 64),
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
