use super::auth::ensure_add_uri_token;
use super::types::{positional_params, strip_token_param, RpcFault};
use crate::app::HttpAppState;
use crate::runtime::{broadcast_tasks_snapshot, ensure_aria2_ready};
use crate::tasks::repository::SqliteTaskRepository;
use crate::tasks::service::{RuntimeGuard, TaskService};
use crate::tasks::{
    CreateDownloadTaskRequest, CreateTaskAdvancedOptions, DownloadTaskSourceType,
    DownloadTaskStartMode,
};
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug)]
pub(super) struct AddUriCommand {
    pub(super) url: String,
    pub(super) save_dir: Option<String>,
    pub(super) file_name: Option<String>,
    pub(super) aria2_options: serde_json::Map<String, Value>,
}

pub(super) async fn add_uri(state: &Arc<HttpAppState>, params: &Value) -> Result<String, RpcFault> {
    ensure_add_uri_token(state, params).await?;
    let command = parse_add_uri_command(params)?;
    let save_dir = match command.save_dir {
        Some(save_dir) => save_dir,
        None => default_save_dir(state)?,
    };
    ensure_authorized_save_dir(state, &save_dir)?;

    let service = TaskService::new(
        Box::new(SqliteTaskRepository::new(&state.core.database.pool)),
        &state.core.download_tasks,
        &state.core.next_task_id,
        &state.core.debug_logs,
        RuntimeGuard::new(&state.core.shutdown),
    );
    service
        .ensure_not_exiting()
        .map_err(RpcFault::server_error)?;

    let config = ensure_aria2_ready(state)
        .await
        .map_err(RpcFault::server_error)?;
    let task = service
        .create_download_task(
            &config,
            CreateDownloadTaskRequest {
                url: command.url,
                file_name: command.file_name,
                save_dir: Some(save_dir),
                source_type: DownloadTaskSourceType::Url,
                start_mode: DownloadTaskStartMode::Now,
                category: None,
                advanced_options: CreateTaskAdvancedOptions::default(),
                aria2_options: command.aria2_options,
            },
        )
        .await
        .map_err(RpcFault::server_error)?;
    broadcast_tasks_snapshot(state).map_err(RpcFault::server_error)?;

    task.gid
        .filter(|gid| !gid.trim().is_empty())
        .ok_or_else(|| RpcFault::server_error("创建下载任务成功，但响应缺少 GID"))
}

pub(super) fn parse_add_uri_command(params: &Value) -> Result<AddUriCommand, RpcFault> {
    let params = positional_params(params)?;
    let params = strip_token_param(params);
    let uris = params
        .first()
        .ok_or_else(|| RpcFault::invalid_params("aria2.addUri requires URI list"))?;
    let url = first_uri(uris)?;
    let options = params.get(1).and_then(Value::as_object);

    Ok(AddUriCommand {
        url,
        save_dir: options.and_then(|options| string_option(options.get("dir"))),
        file_name: options.and_then(|options| string_option(options.get("out"))),
        aria2_options: options.map(collect_aria2_options).unwrap_or_default(),
    })
}

fn collect_aria2_options(
    options: &serde_json::Map<String, Value>,
) -> serde_json::Map<String, Value> {
    const PASSTHROUGH_OPTIONS: &[&str] = &[
        "allow-overwrite",
        "auto-file-renaming",
        "check-certificate",
        "connect-timeout",
        "continue",
        "header",
        "lowest-speed-limit",
        "max-connection-per-server",
        "max-download-limit",
        "max-file-not-found",
        "max-tries",
        "min-split-size",
        "referer",
        "retry-wait",
        "split",
        "timeout",
        "user-agent",
    ];

    options
        .iter()
        .filter(|(key, _)| PASSTHROUGH_OPTIONS.contains(&key.as_str()))
        .filter_map(|(key, value)| {
            normalize_aria2_option_value(value).map(|value| (key.clone(), value))
        })
        .collect()
}

fn normalize_aria2_option_value(value: &Value) -> Option<Value> {
    match value {
        Value::Null | Value::Object(_) => None,
        Value::String(value) => {
            let value = value.trim();
            if value.is_empty() {
                None
            } else {
                Some(Value::String(value.to_string()))
            }
        }
        Value::Array(items) => {
            let normalized = items
                .iter()
                .filter_map(|item| normalize_aria2_option_value(item))
                .collect::<Vec<_>>();
            if normalized.is_empty() {
                None
            } else {
                Some(Value::Array(normalized))
            }
        }
        Value::Bool(_) | Value::Number(_) => Some(value.clone()),
    }
}

fn first_uri(value: &Value) -> Result<String, RpcFault> {
    let uri = match value {
        Value::Array(uris) => uris.first().and_then(Value::as_str),
        Value::String(uri) => Some(uri.as_str()),
        _ => None,
    };
    uri.map(str::trim)
        .filter(|uri| !uri.is_empty())
        .map(str::to_string)
        .ok_or_else(|| RpcFault::invalid_params("aria2.addUri requires a non-empty URI"))
}

fn string_option(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn default_save_dir(state: &HttpAppState) -> Result<String, RpcFault> {
    crate::storage::load_default_download_dir(
        &state.runtime.accessible_paths_path,
        &state.runtime.app_data_dir,
    )
    .map_err(RpcFault::server_error)
}

fn ensure_authorized_save_dir(state: &HttpAppState, save_dir: &str) -> Result<(), RpcFault> {
    let accessible_paths =
        crate::storage::load_accessible_paths(&state.runtime.accessible_paths_path)
            .map_err(RpcFault::server_error)?;
    if accessible_paths.is_empty() {
        return Err(RpcFault::invalid_params(
            "未检测到已授权目录，请先在飞牛应用设置中添加读写文件夹授权",
        ));
    }
    if !accessible_paths.iter().any(|path| path == save_dir) {
        return Err(RpcFault::invalid_params("保存目录不在飞牛已授权目录列表中"));
    }
    Ok(())
}
