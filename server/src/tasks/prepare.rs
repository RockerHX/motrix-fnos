use crate::debug_logs::DebugLogStore;
use crate::tasks::{CreateDownloadTaskRequest, PreparedDownloadTask, DEFAULT_TASK_CATEGORY};
use std::path::{Path, PathBuf};
use std::{env, fs};

use super::{current_timestamp_ms, log_error, log_info, redact_url_for_log};

pub fn prepare_task(request: CreateDownloadTaskRequest) -> Result<PreparedDownloadTask, String> {
    prepare_task_inner(request, None)
}

pub fn prepare_task_with_logs(
    request: CreateDownloadTaskRequest,
    debug_logs: &DebugLogStore,
) -> Result<PreparedDownloadTask, String> {
    prepare_task_inner(request, Some(debug_logs))
}

fn prepare_task_inner(
    request: CreateDownloadTaskRequest,
    debug_logs: Option<&DebugLogStore>,
) -> Result<PreparedDownloadTask, String> {
    let url = match normalize_required(&request.url, "下载链接不能为空") {
        Ok(url) => url,
        Err(error) => {
            log_error(debug_logs, "tasks.create", &error);
            return Err(error);
        }
    };
    if let Err(error) = validate_http_url(&url) {
        log_error(debug_logs, "tasks.create", &error);
        return Err(error);
    }

    let file_name = normalize_optional(request.file_name).unwrap_or_else(|| infer_file_name(&url));
    let save_dir = resolve_save_dir_with_logs(normalize_optional(request.save_dir), debug_logs)?;
    let category =
        normalize_optional(request.category).unwrap_or_else(|| DEFAULT_TASK_CATEGORY.to_string());
    log_info(
        debug_logs,
        "tasks.create",
        format!(
            "下载任务参数已准备，URL {}，文件名 {}，保存目录 {}，分类 {}",
            redact_url_for_log(&url),
            file_name,
            save_dir,
            category
        ),
    );

    Ok(PreparedDownloadTask {
        file_name,
        save_dir,
        category,
        url,
        source_type: request.source_type,
        start_mode: request.start_mode,
        advanced_options: request.advanced_options,
        aria2_options: request.aria2_options,
    })
}

fn normalize_required(value: &str, empty_message: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(empty_message.to_string());
    }

    Ok(trimmed.to_string())
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub(crate) fn resolve_save_dir_with_logs(
    input: Option<String>,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    let source = if input.is_some() {
        "自定义"
    } else {
        "默认"
    };
    let path = match input {
        Some(path) => expand_home_dir(&path)?,
        None => default_download_dir()?,
    };
    log_info(
        debug_logs,
        "tasks.path",
        format!("解析{}下载目录：{}", source, path.display()),
    );

    if let Err(error) = fs::create_dir_all(&path) {
        let error = format!("创建下载目录失败：{}（{}）", path.display(), error);
        log_error(debug_logs, "tasks.path", &error);
        return Err(error);
    }

    if !path.is_dir() {
        let error = format!("下载目录不是有效文件夹：{}", path.display());
        log_error(debug_logs, "tasks.path", &error);
        return Err(error);
    }

    if let Err(error) = verify_save_dir_writable(&path) {
        log_error(debug_logs, "tasks.path", &error);
        return Err(error);
    }

    log_info(
        debug_logs,
        "tasks.path",
        format!("下载目录可用：{}", path.display()),
    );

    Ok(path.display().to_string())
}

fn verify_save_dir_writable(path: &Path) -> Result<(), String> {
    let probe_path = path.join(format!(
        ".motrix-fnos-write-test-{}-{}",
        std::process::id(),
        current_timestamp_ms()
    ));

    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe_path)
    {
        Ok(_) => {
            if let Err(error) = fs::remove_file(&probe_path) {
                return Err(format!(
                    "下载目录写入探测文件清理失败：{}（{}）",
                    probe_path.display(),
                    error
                ));
            }
            Ok(())
        }
        Err(error) => Err(format!(
            "下载目录不可写，应用无法在该目录创建文件：{}（{}）",
            path.display(),
            error
        )),
    }
}

pub(crate) fn default_download_dir() -> Result<PathBuf, String> {
    Ok(home_dir()?.join("Downloads"))
}

pub fn default_download_dir_string() -> Result<String, String> {
    Ok(default_download_dir()?.display().to_string())
}

pub(crate) fn expand_home_dir(path: &str) -> Result<PathBuf, String> {
    if path == "~" {
        return home_dir();
    }

    if let Some(rest) = path.strip_prefix("~/") {
        return Ok(home_dir()?.join(rest));
    }

    Ok(PathBuf::from(path))
}

fn home_dir() -> Result<PathBuf, String> {
    env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "无法读取当前用户目录，不能确定默认下载目录".to_string())
}

fn validate_http_url(url: &str) -> Result<(), String> {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Ok(());
    }

    Err("当前仅支持 HTTP / HTTPS 下载链接".to_string())
}

fn infer_file_name(url: &str) -> String {
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .trim_end_matches('/');

    path.rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("未命名下载任务")
        .to_string()
}
