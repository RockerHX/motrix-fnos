use super::transport::rpc_params;
use crate::aria2::Aria2RpcClient;
use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use crate::tasks::{log_error, Aria2TaskStatus};

pub(crate) async fn tell_status(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<Aria2TaskStatus, String> {
    let request_body = super::build_tell_status_request(config, gid);
    let status = match client
        .request::<Aria2TaskStatus>(config, &request_body)
        .await
        .and_then(|response| response.into_result())
    {
        Ok(status) => status,
        Err(error) => {
            let error = format!("同步 Aria2 任务状态失败：{}", error);
            log_error(
                debug_logs,
                "aria2.tellStatus",
                format!("GID {} {}", gid, error),
            );
            return Err(error);
        }
    };
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

pub(crate) async fn task_exists(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<bool, String> {
    match tell_status(client, config, gid, debug_logs).await {
        Ok(_) => Ok(true),
        Err(error) if crate::tasks::is_stale_aria2_gid_error(&error) => Ok(false),
        Err(error) => Err(error),
    }
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
