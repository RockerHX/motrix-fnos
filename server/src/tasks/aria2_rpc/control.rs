use super::transport::rpc_params;
use crate::aria2::{Aria2RpcClient, Aria2RpcError};
use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use crate::tasks::{log_error, log_info};

#[derive(Debug)]
pub enum Aria2TaskOptionError {
    OutcomeUnknown(String),
    Failed(String),
}

impl Aria2TaskOptionError {
    pub fn is_outcome_unknown(&self) -> bool {
        matches!(self, Self::OutcomeUnknown(_))
    }
}

impl std::fmt::Display for Aria2TaskOptionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutcomeUnknown(message) | Self::Failed(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for Aria2TaskOptionError {}

pub async fn pause_task(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    pause_task_with_request_id(client, config, gid, None, debug_logs).await
}

pub async fn pause_task_with_request_id(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    gid: &str,
    request_id: Option<&str>,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    send_gid_control_request(
        client,
        config,
        gid,
        "aria2.pause",
        request_id.unwrap_or("motrix-fnos-pause"),
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
    unpause_task_with_request_id(client, config, gid, None, debug_logs).await
}

pub async fn unpause_task_with_request_id(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    gid: &str,
    request_id: Option<&str>,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    send_gid_control_request(
        client,
        config,
        gid,
        "aria2.unpause",
        request_id.unwrap_or("motrix-fnos-unpause"),
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
    change_task_options_with_request_id(client, config, gid, options, None, debug_logs)
        .await
        .map_err(|error| error.to_string())
}

pub async fn change_task_options_with_request_id(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    gid: &str,
    options: serde_json::Map<String, serde_json::Value>,
    request_id: Option<&str>,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, Aria2TaskOptionError> {
    let request_body = build_change_option_request_with_id(
        config,
        gid,
        options,
        request_id.unwrap_or("motrix-fnos-change-option"),
    );
    match client
        .request::<String>(config, &request_body)
        .await
        .and_then(|response| response.into_optional_result())
    {
        Ok(Some(gid)) if !gid.trim().is_empty() => Ok(gid),
        Ok(_) => Ok(gid.to_string()),
        Err(error) => {
            let outcome_unknown = matches!(
                &error,
                Aria2RpcError::OutcomeUnknown(_)
                    | Aria2RpcError::HttpStatus(_)
                    | Aria2RpcError::InvalidResponse(_)
            );
            let message = format!("更新任务选项失败：{}", error);
            log_error(debug_logs, "aria2.changeOption", &message);
            Err(if outcome_unknown {
                Aria2TaskOptionError::OutcomeUnknown(message)
            } else {
                Aria2TaskOptionError::Failed(message)
            })
        }
    }
}

pub async fn remove_task(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    remove_task_with_request_id(client, config, gid, None, debug_logs).await
}

pub async fn remove_task_with_request_id(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    gid: &str,
    request_id: Option<&str>,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    let request_id = request_id.unwrap_or("motrix-fnos-remove");
    match send_gid_control_request(
        client,
        config,
        gid,
        "aria2.remove",
        request_id,
        "删除任务",
        debug_logs,
    )
    .await
    {
        Ok(result_gid) => Ok(result_gid),
        Err(error) if is_aria2_outcome_unknown_error(&error) => Err(error),
        Err(error) => {
            log_info(
                debug_logs,
                "aria2.removeDownloadResult",
                format!(
                    "aria2.remove 未完成，尝试清理已停止任务结果，GID {}：{}",
                    gid, error
                ),
            );
            let result_request_id = format!("{request_id}:remove-result");
            send_gid_control_request(
                client,
                config,
                gid,
                "aria2.removeDownloadResult",
                &result_request_id,
                "删除任务结果",
                debug_logs,
            )
            .await
        }
    }
}

pub(crate) fn is_aria2_outcome_unknown_error(error: &str) -> bool {
    error.contains("Aria2 RPC 结果未知")
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
    build_change_option_request_with_id(config, gid, options, "motrix-fnos-change-option")
}

pub(crate) fn build_change_option_request_with_id(
    config: &Aria2Config,
    gid: &str,
    options: serde_json::Map<String, serde_json::Value>,
    request_id: &str,
) -> serde_json::Value {
    let mut params = rpc_params(config);
    params.push(serde_json::json!(gid));
    params.push(serde_json::Value::Object(options));

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "aria2.changeOption",
        "params": params,
    })
}

#[cfg(test)]
mod tests;
