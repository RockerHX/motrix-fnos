use super::transport::{rpc_params, TellStatusResponse};
use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use crate::tasks::{log_error, Aria2TaskStatus};

pub(crate) async fn tell_status(
    client: &reqwest::Client,
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<Aria2TaskStatus, String> {
    let request_body = super::build_tell_status_request(config, gid);
    let response = match client
        .post(config.rpc_url())
        .json(&request_body)
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => {
            let error = "同步任务状态失败：无法连接 Aria2 RPC".to_string();
            log_error(
                debug_logs,
                "aria2.tellStatus",
                format!("GID {} {}", gid, error),
            );
            return Err(error);
        }
    };

    let rpc_response = match response.json::<TellStatusResponse>().await {
        Ok(response) => response,
        Err(error) => {
            let error = format!("同步 Aria2 任务状态解析失败：{}", error);
            log_error(
                debug_logs,
                "aria2.tellStatus",
                format!("GID {} {}", gid, error),
            );
            return Err(error);
        }
    };

    if let Some(error) = rpc_response.error {
        let error = format!("同步 Aria2 任务状态失败：{}", error.message);
        log_error(
            debug_logs,
            "aria2.tellStatus",
            format!("GID {} {}", gid, error),
        );
        return Err(error);
    }

    let status = rpc_response
        .result
        .ok_or_else(|| "同步 Aria2 任务状态失败：响应缺少任务状态".to_string())?;
    if crate::tasks::is_aria2_status_error(&status) {
        log_error(
            debug_logs,
            "aria2.tellStatus",
            format!(
                "GID {} 返回错误状态，错误码 {}，原因 {}",
                gid,
                status.error_code.as_deref().unwrap_or("-"),
                status.error_message.as_deref().unwrap_or("未知错误")
            ),
        );
    }
    Ok(status)
}

pub(crate) fn build_tell_status_request(config: &Aria2Config, gid: &str) -> serde_json::Value {
    let mut params = rpc_params(config);
    params.push(serde_json::json!(gid));
    params.push(status_fields());

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": "motrix-fnos-tell-status",
        "method": "aria2.tellStatus",
        "params": params,
    })
}

pub(crate) fn build_tell_many_request(config: &Aria2Config, method: &str) -> serde_json::Value {
    let mut params = rpc_params(config);
    if method != "aria2.tellActive" {
        params.push(serde_json::json!(0));
        params.push(serde_json::json!(1000));
    }
    params.push(status_fields());

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": format!("motrix-fnos-{}", method.replace('.', "-")),
        "method": method,
        "params": params,
    })
}

fn status_fields() -> serde_json::Value {
    serde_json::json!([
        "gid",
        "status",
        "totalLength",
        "completedLength",
        "downloadSpeed",
        "connections",
        "numSeeders",
        "seeder",
        "bittorrent",
        "infoHash",
        "followedBy",
        "following",
        "belongsTo",
        "errorCode",
        "errorMessage",
        "dir",
        "files"
    ])
}
