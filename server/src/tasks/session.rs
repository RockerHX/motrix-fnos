use crate::aria2::Aria2RpcClient;
use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use crate::tasks::prepare::validate_url_output_file_name;
use crate::tasks::{
    change_task_options_with_request_id, get_task_options, Aria2TaskOptionError, Aria2TaskRequest,
    DownloadTask, DownloadTaskSourceType, TaskMemoryState,
};

use super::aria2_rpc::build_tell_many_request;
use super::{log_error, log_info, Aria2TaskStatus};
use std::collections::BTreeSet;
use std::path::Path;

mod native;
pub(crate) use native::reconcile_native_task;

pub(crate) async fn validate_url_task_output_before_resume(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    task: &DownloadTask,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<(), String> {
    if task.source_type != DownloadTaskSourceType::Url {
        return Ok(());
    }

    let status = super::aria2_rpc::tell_status(client, config, gid, debug_logs).await?;
    if status.status == "error" {
        return Err(status
            .error_message
            .unwrap_or_else(|| "Aria2 任务状态异常".to_string()));
    }

    let options = get_task_options(
        client,
        config,
        gid,
        Some(&format!("motrix-fnos-output-validate-{}", task.id)),
        debug_logs,
    )
    .await
    .map_err(|error| format!("恢复任务前无法读取 Aria2 输出选项：{error}"))?;
    let configured_dir = options
        .get("dir")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "恢复任务前无法确认 Aria2 输出保存目录".to_string())?;
    let status_dir = status
        .dir
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "恢复任务前无法确认 Aria2 实际保存目录".to_string())?;
    let expected_dir = normalize_path_for_match(&task.save_dir);
    if normalize_path_for_match(configured_dir) != expected_dir
        || normalize_path_for_match(status_dir) != expected_dir
    {
        return Err("恢复任务前无法确认 Aria2 输出目录安全".to_string());
    }

    let configured_out = match options.get("out") {
        None => None,
        Some(serde_json::Value::String(value)) if value.trim().is_empty() => None,
        Some(serde_json::Value::String(value)) => Some(value.as_str()),
        Some(_) => return Err("恢复任务前无法确认 Aria2 输出文件名".to_string()),
    };
    let configured_out = validated_url_output_name(configured_out, &expected_dir)?;

    let files = status
        .files
        .as_deref()
        .filter(|files| !files.is_empty())
        .ok_or_else(|| "恢复任务前无法确认 Aria2 实际输出路径".to_string())?;
    for file in files {
        let normalized_path = file.path.replace('\\', "/");
        let path = Path::new(&normalized_path);
        if path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
            || std::fs::symlink_metadata(path)
                .is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err("恢复任务前无法确认 Aria2 实际输出路径".to_string());
        }
        let parent = path
            .parent()
            .and_then(Path::to_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "恢复任务前无法确认 Aria2 实际输出路径".to_string())?;
        if normalize_path_for_match(parent) != expected_dir {
            return Err("恢复任务前无法确认 Aria2 实际输出路径".to_string());
        }
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| "恢复任务前无法确认 Aria2 实际输出文件名".to_string())?;
        validate_url_output_file_name(Some(file_name))
            .map_err(|_| "恢复任务前无法确认 Aria2 实际输出文件名".to_string())?;
        if configured_out.is_some_and(|output| output != file_name) {
            return Err("恢复任务前无法确认 Aria2 输出文件名".to_string());
        }
    }

    Ok(())
}

fn validated_url_output_name<'a>(
    output: Option<&'a str>,
    expected_dir: &str,
) -> Result<Option<&'a str>, String> {
    let error = || "恢复任务前无法确认 Aria2 输出文件名".to_string();
    let Some(output) = output else {
        return Ok(None);
    };
    let path = Path::new(output);
    let name = if path.is_absolute() {
        if output.contains(['\\', '\0'])
            || path
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
            || path
                .parent()
                .and_then(Path::to_str)
                .map(normalize_path_for_match)
                .as_deref()
                != Some(expected_dir)
        {
            return Err(error());
        }
        path.file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(error)?
    } else {
        output
    };
    validate_url_output_file_name(Some(name)).map_err(|_| error())?;
    Ok(Some(name))
}

pub async fn sync_session_tasks_from_aria2(
    tasks: &TaskMemoryState,
    client: &Aria2RpcClient,
    config: &Aria2Config,
    app_data_dir: &Path,
    debug_logs: Option<&DebugLogStore>,
) -> Result<Vec<DownloadTask>, String> {
    let session_tasks = list_current_aria2_tasks(client, config, debug_logs).await?;
    native::sync_native_session(tasks, &session_tasks, app_data_dir)
}

pub async fn reconcile_session_task_proxies(
    tasks: &TaskMemoryState,
    client: &Aria2RpcClient,
    config: &Aria2Config,
    debug_logs: Option<&DebugLogStore>,
) -> Result<(), String> {
    let candidates = crate::tasks::list_tasks(tasks)?
        .into_iter()
        .filter(|task| task.engine_generation == crate::config::aria2::EngineGeneration::NativeV2)
        .filter(|task| {
            task.gid
                .as_deref()
                .is_some_and(|gid| !gid.trim().is_empty())
        })
        .collect::<Vec<_>>();
    let mut applied = 0_usize;
    let mut unchanged = 0_usize;

    for task in candidates {
        match reconcile_task_proxy_option(client, config, &task, None, debug_logs).await {
            Ok(true) => applied += 1,
            Ok(false) => unchanged += 1,
            Err(error) => {
                return Err(format!(
                    "恢复任务代理状态失败，任务 ID {}：{}",
                    task.id, error
                ));
            }
        }
    }

    log_info(
        debug_logs,
        "tasks.proxy_reconcile",
        format!(
            "任务代理对账完成：应用 {} 个，无变化 {} 个",
            applied, unchanged
        ),
    );
    Ok(())
}

pub async fn reconcile_task_proxy_option(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    task: &DownloadTask,
    gid_override: Option<&str>,
    debug_logs: Option<&DebugLogStore>,
) -> Result<bool, Aria2TaskOptionError> {
    if task.source_type != DownloadTaskSourceType::Url {
        return Ok(false);
    }
    let gid = gid_override
        .or(task.gid.as_deref())
        .filter(|gid| !gid.trim().is_empty())
        .ok_or_else(|| {
            Aria2TaskOptionError::Failed("任务缺少 Aria2 GID，无法对账代理选项".to_string())
        })?;
    let request_id = format!("motrix-fnos-proxy-reconcile-{}", task.id);
    let current = get_task_options(client, config, gid, Some(&request_id), debug_logs).await?;
    let target = if task.use_proxy {
        task.proxy_binding.effective_proxy_url().ok_or_else(|| {
            Aria2TaskOptionError::Failed("任务要求使用代理，但没有可用的代理配置".to_string())
        })?
    } else {
        ""
    };
    let current_proxy = current
        .get("all-proxy")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if current_proxy == target {
        return Ok(false);
    }

    let options = serde_json::Map::from_iter([(
        "all-proxy".to_string(),
        serde_json::Value::String(target.to_string()),
    )]);
    change_task_options_with_request_id(
        client,
        config,
        gid,
        options,
        Some(&format!("{request_id}-apply")),
        debug_logs,
    )
    .await?;
    Ok(true)
}

async fn list_current_aria2_tasks(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    debug_logs: Option<&DebugLogStore>,
) -> Result<Vec<Aria2TaskStatus>, String> {
    let mut tasks = Vec::new();
    for method in ["aria2.tellActive", "aria2.tellWaiting", "aria2.tellStopped"] {
        match tell_many_tasks(client, config, method).await {
            Ok(mut result) => tasks.append(&mut result),
            Err(error) => {
                log_error(debug_logs, "tasks.restore", &error);
                return Err(error);
            }
        }
    }
    Ok(tasks)
}

pub(crate) async fn find_aria2_task_for_request(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    request: &Aria2TaskRequest,
    excluded_gids: &BTreeSet<String>,
    debug_logs: Option<&DebugLogStore>,
) -> Result<Option<String>, String> {
    let session_tasks = list_current_aria2_tasks(client, config, debug_logs).await?;
    let candidates = matching_aria2_task_gids(&session_tasks, request, excluded_gids);

    if candidates.len() == 1 {
        return Ok(candidates.into_iter().next());
    }

    if candidates.len() > 1 {
        log_info(
            debug_logs,
            "tasks.operation",
            format!(
                "Aria2 请求对账匹配到多个任务，operationId {}，不自动选择",
                request.request_id
            ),
        );
    }
    Ok(None)
}

pub(crate) fn matching_aria2_task_gids(
    session_tasks: &[Aria2TaskStatus],
    request: &Aria2TaskRequest,
    excluded_gids: &BTreeSet<String>,
) -> BTreeSet<String> {
    session_tasks
        .iter()
        .filter(|task| session_task_matches_aria2_request(task, request))
        .filter_map(|task| task.gid.as_deref())
        .filter(|gid| !gid.trim().is_empty() && !excluded_gids.contains(*gid))
        .map(str::to_string)
        .collect()
}

async fn tell_many_tasks(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    method: &str,
) -> Result<Vec<Aria2TaskStatus>, String> {
    let mut request_body = build_tell_many_request(config, method);
    let mut tasks = Vec::new();
    let mut seen = BTreeSet::new();
    loop {
        let page = client
            .request::<Vec<Aria2TaskStatus>>(config, &request_body)
            .await
            .and_then(|response| response.into_result())
            .map_err(|error| format!("读取 Aria2 session 任务失败：{error}"))?;
        for task in &page {
            if let Some(gid) = &task.gid {
                if !seen.insert(gid.clone()) {
                    return Err("Aria2 session 分页返回重复 GID".to_string());
                }
            }
        }
        let count = page.len();
        tasks.extend(page);
        if method == "aria2.tellActive" || count < 1000 {
            break;
        }
        let params = request_body["params"].as_array_mut().unwrap();
        let offset_index = params.len() - 3;
        params[offset_index] = serde_json::json!(tasks.len());
    }
    Ok(tasks)
}

fn session_task_urls(session_task: &Aria2TaskStatus) -> Vec<String> {
    session_task
        .files
        .as_ref()
        .into_iter()
        .flatten()
        .flat_map(|file| file.uris.iter())
        .map(|uri| uri.uri.trim())
        .filter(|uri| !uri.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn session_task_matches_aria2_request(
    session_task: &Aria2TaskStatus,
    request: &Aria2TaskRequest,
) -> bool {
    let dir_matches = session_task
        .dir
        .as_deref()
        .filter(|dir| !dir.trim().is_empty())
        .map(|dir| normalize_path_for_match(dir) == normalize_path_for_match(&request.save_dir))
        .unwrap_or(false);
    if !dir_matches {
        return false;
    }

    if request.source_url.starts_with("torrent:") {
        return session_task.bittorrent.is_some();
    }

    let url_matches = session_task_urls(session_task)
        .iter()
        .any(|url| url == &request.source_url);
    let file_name_matches = session_task.files.as_ref().is_some_and(|files| {
        files.iter().any(|file| {
            normalize_path_for_match(&file.path).rsplit('/').next()
                == Some(request.file_name.as_str())
        })
    });
    url_matches && file_name_matches
}

fn normalize_path_for_match(path: &str) -> String {
    path.replace('\\', "/").trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests;
