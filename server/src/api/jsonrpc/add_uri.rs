use super::auth::ensure_add_uri_token;
use super::types::{positional_params, strip_token_param, RpcFault};
use super::JsonRpcAccess;
use crate::api::tasks::task_service;
use crate::app::HttpAppState;
use crate::runtime::{broadcast_tasks_snapshot, ensure_aria2_ready};
use crate::tasks::{
    sanitize_aria2_options, CreateDownloadTaskRequest, CreateTaskAdvancedOptions,
    DownloadTaskSourceType, DownloadTaskStartMode,
};
use serde_json::Value;
use std::sync::Arc;

pub(super) struct AddUriCommand {
    pub(super) url: String,
    pub(super) source_type: DownloadTaskSourceType,
    pub(super) save_dir: Option<String>,
    pub(super) file_name: Option<String>,
    pub(super) aria2_options: serde_json::Map<String, Value>,
}

impl std::fmt::Debug for AddUriCommand {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AddUriCommand")
            .field("url", &self.url)
            .field("source_type", &self.source_type)
            .field("save_dir", &self.save_dir)
            .field("file_name", &self.file_name)
            .field(
                "aria2_option_keys",
                &self.aria2_options.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

pub(super) async fn add_uri(
    state: &Arc<HttpAppState>,
    access: JsonRpcAccess,
    params: &Value,
) -> Result<String, RpcFault> {
    ensure_add_uri_token(state, access, params).await?;
    let command = parse_add_uri_command(params)?;
    let save_dir = match command.save_dir {
        Some(save_dir) => save_dir,
        None => default_save_dir(state)?,
    };
    let save_dir = authorized_save_dir(state, &save_dir)?;

    let service = task_service(state);
    service
        .ensure_not_exiting()
        .map_err(RpcFault::server_error)?;

    let config = ensure_aria2_ready(state).await.map_err(|error| {
        if error.contains("生命周期转换超时") || error.contains("生命周期请求被拒绝")
        {
            RpcFault::aria2_busy(error)
        } else {
            RpcFault::server_error(error)
        }
    })?;
    let task = service
        .create_download_task(
            &config,
            CreateDownloadTaskRequest {
                url: command.url,
                file_name: command.file_name,
                save_dir: Some(save_dir),
                source_type: command.source_type,
                start_mode: DownloadTaskStartMode::Now,
                category: None,
                advanced_options: CreateTaskAdvancedOptions::default(),
                aria2_options: command.aria2_options,
            },
        )
        .await
        .map_err(classify_create_error)?;
    broadcast_tasks_snapshot(state).map_err(RpcFault::server_error)?;

    task.gid
        .filter(|gid| !gid.trim().is_empty())
        .ok_or_else(|| RpcFault::server_error("创建下载任务成功，但响应缺少 GID"))
}

fn classify_create_error(error: String) -> RpcFault {
    if error.contains("代理选择冲突")
        || error.contains("代理地址")
        || error.contains("代理协议")
        || error.contains("代理端口")
    {
        return RpcFault::invalid_params(error);
    }
    RpcFault::server_error(error)
}

pub(super) fn parse_add_uri_command(params: &Value) -> Result<AddUriCommand, RpcFault> {
    let params = positional_params(params)?;
    let params = strip_token_param(params);
    let uris = params
        .first()
        .ok_or_else(|| RpcFault::invalid_params("aria2.addUri requires URI list"))?;
    let url = first_uri(uris)?;
    let options = params.get(1).and_then(Value::as_object);

    let mut aria2_options = options.map(sanitize_aria2_options).unwrap_or_default();
    match aria2_options.get("all-proxy") {
        Some(Value::String(proxy_url)) => {
            let normalized =
                crate::settings::proxy::normalize_proxy_url(proxy_url).map_err(|error| {
                    let message = match error {
                        crate::settings::proxy::DownloadProxyServiceError::InvalidUrl(message) => {
                            message
                        }
                        _ => "代理地址校验失败",
                    };
                    RpcFault::invalid_params(message)
                })?;
            aria2_options.insert("all-proxy".to_string(), Value::String(normalized));
        }
        Some(_) => return Err(RpcFault::invalid_params("代理地址必须是字符串")),
        None => {}
    }

    Ok(AddUriCommand {
        source_type: detect_source_type(&url),
        url,
        save_dir: options.and_then(|options| string_option(options.get("dir"))),
        file_name: options.and_then(|options| string_option(options.get("out"))),
        aria2_options,
    })
}

fn detect_source_type(url: &str) -> DownloadTaskSourceType {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("magnet:?") {
        DownloadTaskSourceType::Magnet
    } else if lower.starts_with("torrent:") {
        DownloadTaskSourceType::Torrent
    } else {
        DownloadTaskSourceType::Url
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

pub(super) fn authorized_save_dir(
    state: &HttpAppState,
    save_dir: &str,
) -> Result<String, RpcFault> {
    let accessible_paths =
        crate::storage::load_accessible_paths(&state.runtime.accessible_paths_path)
            .map_err(RpcFault::server_error)?;
    let cached_default = state.json_rpc_default_download_dir();
    let resolved = match resolve_authorized_save_dir(save_dir, &accessible_paths) {
        Ok(resolved) => Ok(resolved),
        Err(crate::storage::TaskSaveDirError::Unauthorized)
            if !cached_default.is_empty() && save_dir.trim() == cached_default =>
        {
            let current_default = crate::storage::default_download_dir(
                &accessible_paths,
                &state.runtime.app_data_dir,
            )
            .display()
            .to_string();
            resolve_authorized_save_dir(&current_default, &accessible_paths)
        }
        Err(error) => Err(error),
    };
    resolved
        .inspect(|resolved| {
            state.remember_json_rpc_default_download_dir(resolved);
        })
        .map_err(|error| {
            let message = match error {
                crate::storage::TaskSaveDirError::Required => "请选择已授权的保存目录",
                crate::storage::TaskSaveDirError::NoAccessiblePaths => {
                    "未检测到已授权目录，请先在飞牛应用设置中添加读写文件夹授权"
                }
                crate::storage::TaskSaveDirError::Unauthorized => {
                    "保存目录不在飞牛已授权目录列表中"
                }
            };
            RpcFault::invalid_params(message)
        })
}

pub(super) fn resolve_authorized_save_dir(
    save_dir: &str,
    accessible_paths: &[String],
) -> Result<String, crate::storage::TaskSaveDirError> {
    let save_dir = save_dir.trim();
    match crate::storage::validate_task_save_dir(Some(save_dir), accessible_paths) {
        Ok(()) => {
            return accessible_paths
                .iter()
                .find(|path| path.as_str() == save_dir)
                .cloned()
                .ok_or(crate::storage::TaskSaveDirError::Unauthorized);
        }
        Err(crate::storage::TaskSaveDirError::Unauthorized) => {}
        Err(error) => return Err(error),
    }

    if save_dir.starts_with('/')
        || save_dir.contains('\\')
        || !save_dir
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
    {
        return Err(crate::storage::TaskSaveDirError::Unauthorized);
    }

    let candidate = format!("/{save_dir}");
    let mut matches = accessible_paths
        .iter()
        .filter(|path| path.as_str() == candidate);
    let matched = matches
        .next()
        .cloned()
        .ok_or(crate::storage::TaskSaveDirError::Unauthorized)?;
    if matches.next().is_some() {
        return Err(crate::storage::TaskSaveDirError::Unauthorized);
    }
    Ok(matched)
}
