use super::transport::{rpc_params, GidResponse};
use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use crate::tasks::{log_error, log_info};

pub async fn pause_task(
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    send_gid_control_request(
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
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    send_gid_control_request(
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
    config: &Aria2Config,
    gid: &str,
    options: serde_json::Map<String, serde_json::Value>,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    let request_body = super::build_change_option_request(config, gid, options);
    let response = match reqwest::Client::new()
        .post(config.rpc_url())
        .json(&request_body)
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => {
            let error = "更新任务选项失败：无法连接 Aria2 RPC".to_string();
            log_error(debug_logs, "aria2.changeOption", &error);
            return Err(error);
        }
    };

    let rpc_response = match response.json::<GidResponse>().await {
        Ok(response) => response,
        Err(error) => {
            let error = format!("更新任务选项失败，响应解析失败：{}", error);
            log_error(debug_logs, "aria2.changeOption", &error);
            return Err(error);
        }
    };

    if let Some(error) = rpc_response.error {
        let error = format!("更新任务选项失败：{}", error.message);
        log_error(debug_logs, "aria2.changeOption", &error);
        return Err(error);
    }

    Ok(rpc_response.result.unwrap_or_else(|| gid.to_string()))
}

pub async fn remove_task(
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    match send_gid_control_request(
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
    let response = match reqwest::Client::new()
        .post(config.rpc_url())
        .json(&request_body)
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => {
            let error = format!("{}失败：无法连接 Aria2 RPC", action_label);
            log_error(debug_logs, module, format!("GID {} {}", gid, error));
            return Err(error);
        }
    };

    let rpc_response = match response.json::<GidResponse>().await {
        Ok(response) => response,
        Err(error) => {
            let error = format!("{}失败，响应解析失败：{}", action_label, error);
            log_error(debug_logs, module, format!("GID {} {}", gid, error));
            return Err(error);
        }
    };

    if let Some(error) = rpc_response.error {
        let error = format!("{}失败：{}", action_label, error.message);
        log_error(debug_logs, module, format!("GID {} {}", gid, error));
        return Err(error);
    }

    let result_gid = rpc_response
        .result
        .filter(|gid| !gid.trim().is_empty())
        .ok_or_else(|| format!("{}失败：响应缺少 GID", action_label))?;
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
