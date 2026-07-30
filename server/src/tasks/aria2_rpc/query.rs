use super::transport::rpc_params;
use crate::aria2::Aria2RpcClient;
use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use crate::tasks::{log_error, Aria2TaskStatus};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Aria2ActiveTaskActivity {
    #[serde(default)]
    pub(crate) upload_speed: Value,
    #[serde(default)]
    pub(crate) seeder: Value,
    #[serde(default)]
    pub(crate) bittorrent: Option<Value>,
}

impl Aria2ActiveTaskActivity {
    pub(crate) fn is_bt_uploading(&self) -> bool {
        let is_bittorrent = self
            .bittorrent
            .as_ref()
            .map(|value| !value.is_null())
            .unwrap_or(false);
        (is_bittorrent || value_as_bool(&self.seeder))
            && (value_as_u64(&self.upload_speed) > 0 || value_as_bool(&self.seeder))
    }
}

fn value_as_u64(value: &Value) -> u64 {
    match value {
        Value::Number(number) => number.as_u64().unwrap_or_default(),
        Value::String(text) => text.parse().unwrap_or_default(),
        _ => 0,
    }
}

fn value_as_bool(value: &Value) -> bool {
    match value {
        Value::Bool(value) => *value,
        Value::String(text) => text.eq_ignore_ascii_case("true"),
        _ => false,
    }
}

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

pub(crate) async fn tell_active_task_activity(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    debug_logs: Option<&DebugLogStore>,
) -> Result<Vec<Aria2ActiveTaskActivity>, String> {
    let request_body = super::build_tell_many_request(config, "aria2.tellActive");
    client
        .request::<Vec<Aria2ActiveTaskActivity>>(config, &request_body)
        .await
        .and_then(|response| response.into_optional_result())
        .map(|tasks| tasks.unwrap_or_default())
        .map_err(|error| {
            let message = format!("读取 Aria2 活动任务失败：{}", error);
            log_error(debug_logs, "aria2.tellActive", &message);
            message
        })
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
