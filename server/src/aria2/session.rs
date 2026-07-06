use super::rpc::EmptyJsonRpcResponse;
use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;

pub async fn save_session(
    config: &Aria2Config,
    debug_logs: Option<&DebugLogStore>,
) -> Result<(), String> {
    let request_body = build_save_session_request(config);
    let response = reqwest::Client::new()
        .post(config.rpc_url())
        .json(&request_body)
        .send()
        .await
        .map_err(|error| format!("保存 Aria2 session 失败：无法连接 RPC（{}）", error))?;

    let rpc_response = response
        .json::<EmptyJsonRpcResponse>()
        .await
        .map_err(|error| format!("保存 Aria2 session 失败：响应解析失败（{}）", error))?;

    if let Some(error) = rpc_response.error {
        return Err(format!("保存 Aria2 session 失败：{}", error.message));
    }

    if let Some(debug_logs) = debug_logs {
        debug_logs.info("aria2.session", "Aria2 session 已保存");
    }

    Ok(())
}

fn build_save_session_request(config: &Aria2Config) -> serde_json::Value {
    let mut params = Vec::new();
    if !config.rpc_secret.is_empty() {
        params.push(serde_json::json!(format!("token:{}", config.rpc_secret)));
    }

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": "motrix-fnos-save-session",
        "method": "aria2.saveSession",
        "params": params,
    })
}
