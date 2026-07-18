use crate::debug_logs::DebugLogStore;
use crate::tasks::files::safe_task_path_component;
use crate::tasks::{
    CreateDownloadTaskRequest, CreateTorrentDownloadTaskRequest, DownloadTaskSourceType,
    PreparedDownloadTask, DEFAULT_TASK_CATEGORY,
};
use std::path::{Path, PathBuf};
use std::{env, fs};

use super::{
    current_timestamp_ms, log_error, log_info, redact_url_for_log, sanitize_create_task_options,
};

#[derive(Debug)]
pub(crate) struct PrepareBtDownloadTaskRequest {
    pub source_url: String,
    pub display_name: String,
    pub base_save_dir: String,
    pub source_type: DownloadTaskSourceType,
    pub start_mode: crate::tasks::DownloadTaskStartMode,
    pub category: Option<String>,
    pub advanced_options: crate::tasks::CreateTaskAdvancedOptions,
    pub aria2_options: serde_json::Map<String, serde_json::Value>,
    pub task_kind: &'static str,
}

pub fn prepare_task(request: CreateDownloadTaskRequest) -> Result<PreparedDownloadTask, String> {
    prepare_task_inner(request, None)
}

pub fn prepare_task_with_logs(
    request: CreateDownloadTaskRequest,
    debug_logs: &DebugLogStore,
) -> Result<PreparedDownloadTask, String> {
    prepare_task_inner(request, Some(debug_logs))
}

pub fn prepare_torrent_task_with_logs(
    request: CreateTorrentDownloadTaskRequest,
    debug_logs: &DebugLogStore,
) -> Result<PreparedDownloadTask, String> {
    let torrent_file_name = normalize_required(&request.torrent_file_name, "种子文件名不能为空")?;
    if request.torrent_data.is_empty() {
        return Err("种子文件不能为空".to_string());
    }

    let prepared = prepare_bt_download_task_with_logs(
        PrepareBtDownloadTaskRequest {
            source_url: format!("torrent:{}", torrent_file_name),
            display_name: torrent_display_name(&torrent_file_name),
            base_save_dir: request.save_dir,
            source_type: DownloadTaskSourceType::Torrent,
            start_mode: request.start_mode,
            category: request.category,
            advanced_options: request.advanced_options,
            aria2_options: serde_json::Map::new(),
            task_kind: "种子",
        },
        Some(debug_logs),
    )?;
    log_info(
        Some(debug_logs),
        "tasks.create",
        format!(
            "种子任务参数已准备，文件 {}，任务名 {}，保存目录 {}，分类 {}",
            torrent_file_name, prepared.file_name, prepared.save_dir, prepared.category
        ),
    );

    Ok(prepared)
}

pub(crate) fn prepare_bt_download_task_with_logs(
    request: PrepareBtDownloadTaskRequest,
    debug_logs: Option<&DebugLogStore>,
) -> Result<PreparedDownloadTask, String> {
    let file_name = normalize_required(&request.display_name, "BT 任务名不能为空")?;
    let base_save_dir = resolve_save_dir_with_logs(Some(request.base_save_dir), debug_logs)?;
    let save_dir = create_bt_task_dir(&base_save_dir, &file_name, request.task_kind, debug_logs)?;
    let category =
        normalize_optional(request.category).unwrap_or_else(|| DEFAULT_TASK_CATEGORY.to_string());
    let aria2_options =
        sanitize_create_task_options(&request.advanced_options, &request.aria2_options)?;

    Ok(PreparedDownloadTask {
        file_name,
        output_file_name: None,
        save_dir,
        aria2_save_dir: None,
        category,
        url: request.source_url,
        source_type: request.source_type,
        start_mode: request.start_mode,
        advanced_options: request.advanced_options,
        aria2_options,
    })
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
    if let Err(error) = validate_task_url(request.source_type, &url) {
        log_error(debug_logs, "tasks.create", &error);
        return Err(error);
    }

    let output_file_name = normalize_optional(request.file_name);
    let file_name = output_file_name
        .clone()
        .unwrap_or_else(|| infer_file_name(request.source_type, &url));
    let base_save_dir =
        resolve_save_dir_with_logs(normalize_optional(request.save_dir), debug_logs)?;
    let save_dir = base_save_dir;
    let category =
        normalize_optional(request.category).unwrap_or_else(|| DEFAULT_TASK_CATEGORY.to_string());
    let aria2_options =
        sanitize_create_task_options(&request.advanced_options, &request.aria2_options)?;
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
        output_file_name,
        save_dir,
        aria2_save_dir: None,
        category,
        url,
        source_type: request.source_type,
        start_mode: request.start_mode,
        advanced_options: request.advanced_options,
        aria2_options,
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

fn create_bt_task_dir(
    base_save_dir: &str,
    task_name: &str,
    task_kind: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    let base_dir = Path::new(base_save_dir);
    let safe_name = safe_task_path_component(task_name);

    // 通过 create_dir 原子占用名称并逐号避让，保证返回目录由当前任务独占；安全递归删除依赖这一所有权约束。
    for index in 0..1000 {
        let dir_name = if index == 0 {
            safe_name.clone()
        } else {
            format!("{} ({})", safe_name, index)
        };
        let candidate = base_dir.join(dir_name);
        match fs::create_dir(&candidate) {
            Ok(_) => {
                if let Err(error) = verify_save_dir_writable(&candidate) {
                    log_error(debug_logs, "tasks.path", &error);
                    return Err(error);
                }
                log_info(
                    debug_logs,
                    "tasks.path",
                    format!("创建{}任务专属目录：{}", task_kind, candidate.display()),
                );
                return Ok(candidate.display().to_string());
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                let error = format!(
                    "创建{}任务目录失败：{}（{}）",
                    task_kind,
                    candidate.display(),
                    error
                );
                log_error(debug_logs, "tasks.path", &error);
                return Err(error);
            }
        }
    }

    Err(format!(
        "无法创建{}任务目录，重名目录过多：{}",
        task_kind, safe_name
    ))
}

fn verify_save_dir_writable(path: &Path) -> Result<(), String> {
    let probe_path = path.join(format!(
        ".motrix-fnos-write-test-{}-{}",
        std::process::id(),
        current_timestamp_ms()
    ));

    // 探针必须使用 create_new，绝不能覆盖授权目录中碰巧同名的既有文件。
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

fn validate_task_url(source_type: DownloadTaskSourceType, url: &str) -> Result<(), String> {
    let lower = url.to_ascii_lowercase();
    match source_type {
        DownloadTaskSourceType::Url
            if lower.starts_with("http://") || lower.starts_with("https://") =>
        {
            Ok(())
        }
        DownloadTaskSourceType::Url => Err("当前仅支持 HTTP / HTTPS 下载链接".to_string()),
        DownloadTaskSourceType::Torrent => Err("种子任务必须通过种子文件创建".to_string()),
        DownloadTaskSourceType::Magnet if lower.starts_with("magnet:?") => Ok(()),
        DownloadTaskSourceType::Magnet => Err("请输入有效的磁力链接".to_string()),
    }
}

fn infer_file_name(source_type: DownloadTaskSourceType, url: &str) -> String {
    if source_type == DownloadTaskSourceType::Magnet {
        return "磁力链接任务".to_string();
    }

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

fn torrent_display_name(file_name: &str) -> String {
    let trimmed = file_name.trim();
    trimmed
        .strip_suffix(".torrent")
        .or_else(|| trimmed.strip_suffix(".TORRENT"))
        .filter(|value| !value.is_empty())
        .unwrap_or(trimmed)
        .to_string()
}
