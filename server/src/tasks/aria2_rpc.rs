use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use serde::Deserialize;

use super::{
    log_error, log_info, redact_url_for_log, Aria2TaskStatus, PreparedDownloadTask,
};

#[derive(Debug, Deserialize)]
struct AddUriResponse {
    result: Option<String>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct GidResponse {
    result: Option<String>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TellStatusResponse {
    result: Option<Aria2TaskStatus>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TellManyResponse {
    pub(crate) result: Option<Vec<Aria2TaskStatus>>,
    pub(crate) error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct JsonRpcError {
    pub(crate) message: String,
}

pub async fn add_uri_to_aria2(
    config: &Aria2Config,
    task: &PreparedDownloadTask,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    log_info(
        debug_logs,
        "aria2.addUri",
        format!(
            "开始创建 Aria2 下载任务，URL {}，保存目录 {}",
            redact_url_for_log(&task.url),
            task.save_dir
        ),
    );
    let request_body = build_add_uri_request(config, task);
    let response = match reqwest::Client::new()
        .post(config.rpc_url())
        .json(&request_body)
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => {
            let error = "创建下载任务失败：无法连接 Aria2 RPC，请确认引擎已启动".to_string();
            log_error(debug_logs, "aria2.addUri", &error);
            return Err(error);
        }
    };

    let rpc_response = match response.json::<AddUriResponse>().await {
        Ok(response) => response,
        Err(error) => {
            let error = format!("创建 Aria2 下载任务失败，响应解析失败：{}", error);
            log_error(debug_logs, "aria2.addUri", &error);
            return Err(error);
        }
    };

    if let Some(error) = rpc_response.error {
        let error = format!("创建 Aria2 下载任务失败：{}", error.message);
        log_error(debug_logs, "aria2.addUri", &error);
        return Err(error);
    }

    let gid = rpc_response
        .result
        .filter(|gid| !gid.trim().is_empty())
        .ok_or_else(|| "创建 Aria2 下载任务失败：响应缺少 GID".to_string())?;
    log_info(
        debug_logs,
        "aria2.addUri",
        format!("Aria2 下载任务创建成功，GID {}", gid),
    );
    Ok(gid)
}

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

pub(crate) async fn tell_status(
    client: &reqwest::Client,
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<Aria2TaskStatus, String> {
    let request_body = build_tell_status_request(config, gid);
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
    if super::is_aria2_status_error(&status) {
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
    let mut params = Vec::new();
    if !config.rpc_secret.is_empty() {
        params.push(serde_json::json!(format!("token:{}", config.rpc_secret)));
    }
    params.push(serde_json::json!(gid));
    params.push(serde_json::json!([
        "gid",
        "status",
        "totalLength",
        "completedLength",
        "downloadSpeed",
        "errorCode",
        "errorMessage",
        "dir",
        "files"
    ]));

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": "motrix-fnos-tell-status",
        "method": "aria2.tellStatus",
        "params": params,
    })
}

pub(crate) fn build_tell_many_request(config: &Aria2Config, method: &str) -> serde_json::Value {
    let mut params = Vec::new();
    if !config.rpc_secret.is_empty() {
        params.push(serde_json::json!(format!("token:{}", config.rpc_secret)));
    }
    if method != "aria2.tellActive" {
        params.push(serde_json::json!(0));
        params.push(serde_json::json!(1000));
    }
    params.push(serde_json::json!([
        "gid",
        "status",
        "totalLength",
        "completedLength",
        "downloadSpeed",
        "errorCode",
        "errorMessage",
        "dir",
        "files"
    ]));

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": format!("motrix-fnos-{}", method.replace('.', "-")),
        "method": method,
        "params": params,
    })
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
    let request_body = build_gid_control_request(config, gid, method, request_id);
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
    let mut params = Vec::new();
    if !config.rpc_secret.is_empty() {
        params.push(serde_json::json!(format!("token:{}", config.rpc_secret)));
    }
    params.push(serde_json::json!(gid));

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": method,
        "params": params,
    })
}

pub(crate) fn build_add_uri_request(
    config: &Aria2Config,
    task: &PreparedDownloadTask,
) -> serde_json::Value {
    let mut params = Vec::new();
    if !config.rpc_secret.is_empty() {
        params.push(serde_json::json!(format!("token:{}", config.rpc_secret)));
    }

    params.push(serde_json::json!([task.url.clone()]));

    let mut options = serde_json::Map::new();
    for (key, value) in task.aria2_options.clone() {
        options.insert(key, value);
    }
    options.insert("dir".to_string(), serde_json::json!(task.save_dir));
    if !task.file_name.trim().is_empty() {
        options.insert("out".to_string(), serde_json::json!(task.file_name));
    }
    params.push(serde_json::Value::Object(options));

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": "motrix-fnos-add-uri",
        "method": "aria2.addUri",
        "params": params,
    })
}
