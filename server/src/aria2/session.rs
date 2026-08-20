use super::rpc::Aria2RpcClient;
use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;

pub async fn save_session(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    debug_logs: Option<&DebugLogStore>,
) -> Result<(), String> {
    let request_body = build_save_session_request(config);
    client
        .request::<serde_json::Value>(config, &request_body)
        .await
        .and_then(|response| response.into_result())
        .map_err(|error| format!("保存 Aria2 session 失败：{}", error))?;

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
