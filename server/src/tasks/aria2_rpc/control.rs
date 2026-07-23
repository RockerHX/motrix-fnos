use super::transport::rpc_params;
use crate::aria2::{Aria2RpcClient, Aria2RpcError};
use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use crate::tasks::{log_error, log_info};

pub async fn pause_task(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    send_gid_control_request(
        client,
        config,
        gid,
        "aria2.pause",
        "motrix-fnos-pause",
        "暂停任务",
        debug_logs,
    )
    .await
}

pub async fn unpause_task(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    send_gid_control_request(
        client,
        config,
        gid,
        "aria2.unpause",
        "motrix-fnos-unpause",
        "恢复任务",
        debug_logs,
    )
    .await
}

pub async fn change_task_options(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    gid: &str,
    options: serde_json::Map<String, serde_json::Value>,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    let request_body = super::build_change_option_request(config, gid, options);
    match client
        .request::<String>(config, &request_body)
        .await
        .and_then(|response| response.into_optional_result())
    {
        Ok(Some(gid)) if !gid.trim().is_empty() => Ok(gid),
        Ok(_) => Ok(gid.to_string()),
        Err(error) => {
            let error = format!("更新任务选项失败：{}", error);
            log_error(debug_logs, "aria2.changeOption", &error);
            Err(error)
        }
    }
}

pub async fn remove_task(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    match send_gid_control_request(
        client,
        config,
        gid,
        "aria2.remove",
        "motrix-fnos-remove",
        "删除任务",
        debug_logs,
    )
    .await
    {
        Ok(result_gid) => Ok(result_gid),
        Err(error) => {
            log_info(
                debug_logs,
                "aria2.removeDownloadResult",
                format!(
                    "aria2.remove 未完成，尝试清理已停止任务结果，GID {}：{}",
                    gid, error
                ),
            );
            send_gid_control_request(
                client,
                config,
                gid,
                "aria2.removeDownloadResult",
                "motrix-fnos-remove-result",
                "删除任务结果",
                debug_logs,
            )
            .await
        }
    }
}

pub(crate) async fn send_gid_control_request(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    gid: &str,
    method: &str,
    request_id: &str,
    action_label: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    let module = method;
    log_info(
        debug_logs,
        module,
        format!("开始{}，GID {}", action_label, gid),
    );
    let request_body = super::build_gid_control_request(config, gid, method, request_id);
    let result_gid = match client
        .request::<String>(config, &request_body)
        .await
        .and_then(|response| response.into_result())
    {
        Ok(result_gid) if !result_gid.trim().is_empty() => result_gid,
        Ok(_) => {
            let error = format!("{}失败：响应缺少 GID", action_label);
            log_error(debug_logs, module, format!("GID {} {}", gid, error));
            return Err(error);
        }
        Err(error) => {
            let error = if matches!(&error, Aria2RpcError::ConnectionFailed(_)) {
                format!("{}失败：无法连接 Aria2 RPC", action_label)
            } else {
                format!("{}失败：{}", action_label, error)
            };
            log_error(debug_logs, module, format!("GID {} {}", gid, error));
            return Err(error);
        }
    };
    log_info(
        debug_logs,
        module,
        format!("{}成功，GID {}", action_label, result_gid),
    );
    Ok(result_gid)
}

pub(crate) fn build_gid_control_request(
    config: &Aria2Config,
    gid: &str,
    method: &str,
    request_id: &str,
) -> serde_json::Value {
    let mut params = rpc_params(config);
    params.push(serde_json::json!(gid));

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": method,
        "params": params,
    })
}

pub(crate) fn build_change_option_request(
    config: &Aria2Config,
    gid: &str,
    options: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let mut params = rpc_params(config);
    params.push(serde_json::json!(gid));
    params.push(serde_json::Value::Object(options));

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": "motrix-fnos-change-option",
        "method": "aria2.changeOption",
        "params": params,
    })
}

#[cfg(test)]
mod tests;
