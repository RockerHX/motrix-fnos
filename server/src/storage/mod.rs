use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

mod display_paths;
mod shared_access;

pub(crate) use display_paths::display_paths;
pub(crate) use shared_access::{refresh_accessible_paths_from_fnos, AccessiblePathsRefreshError};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccessiblePathsResponse {
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DisplayPath {
    pub path: String,
    pub display_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DisplayAccessiblePathsResponse {
    pub paths: Vec<DisplayPath>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskSaveDirError {
    Required,
    NoAccessiblePaths,
    Unauthorized,
    PrepareFailed(String),
}

pub fn load_accessible_paths(accessible_paths_path: &Path) -> Result<Vec<String>, String> {
    if accessible_paths_path.is_file() {
        let content = std::fs::read_to_string(accessible_paths_path)
            .map_err(|error| format!("读取授权目录列表失败：{}", error))?;
        let response = serde_json::from_str::<AccessiblePathsResponse>(&content)
            .map_err(|error| format!("解析授权目录列表失败：{}", error))?;
        return Ok(normalize_paths(response.paths));
    }

    Ok(Vec::new())
}

pub fn default_download_dir(accessible_paths: &[String], app_data_dir: &Path) -> PathBuf {
    if let Some(path) = accessible_paths
        .iter()
        .map(|path| path.trim())
        .find(|path| is_data_path(path))
    {
        return PathBuf::from(path);
    }

    if let Some(path) = accessible_paths
        .iter()
        .map(|path| path.trim())
        .find(|path| !path.is_empty())
    {
        return PathBuf::from(path);
    }

    app_data_dir.to_path_buf()
}

pub fn load_default_download_dir(
    accessible_paths_path: &Path,
    app_data_dir: &Path,
) -> Result<String, String> {
    let accessible_paths = load_accessible_paths(accessible_paths_path)?;
    Ok(default_download_dir(&accessible_paths, app_data_dir)
        .display()
        .to_string())
}

pub fn validate_default_download_dir(
    default_download_dir: &str,
    accessible_paths: &[String],
    app_data_dir: &Path,
) -> Result<(), String> {
    let default_download_dir = default_download_dir.trim();
    if default_download_dir.is_empty() {
        return Err("默认下载目录不能为空".to_string());
    }

    if accessible_paths.is_empty() {
        let app_data_dir = app_data_dir.display().to_string();
        if default_download_dir == app_data_dir {
            return Ok(());
        }
        return Err("默认下载目录不在已授权目录列表中".to_string());
    }

    if accessible_paths
        .iter()
        .any(|path| path == default_download_dir)
    {
        return Ok(());
    }

    Err("默认下载目录不在已授权目录列表中".to_string())
}

pub fn validate_task_save_dir(
    save_dir: Option<&str>,
    accessible_paths: &[String],
) -> Result<(), TaskSaveDirError> {
    let save_dir = save_dir
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(TaskSaveDirError::Required)?;
    if accessible_paths.is_empty() {
        return Err(TaskSaveDirError::NoAccessiblePaths);
    }

    authorized_root_for_save_dir(save_dir, accessible_paths).map(|_| ())
}

pub fn prepare_task_save_dir(
    save_dir: Option<&str>,
    accessible_paths: &[String],
) -> Result<(), TaskSaveDirError> {
    let save_dir = save_dir
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(TaskSaveDirError::Required)?;
    if accessible_paths.is_empty() {
        return Err(TaskSaveDirError::NoAccessiblePaths);
    }

    let authorized_root = authorized_root_for_save_dir(save_dir, accessible_paths)?;
    ensure_safe_save_dir(save_dir, authorized_root)
}

fn authorized_root_for_save_dir<'a>(
    save_dir: &str,
    accessible_paths: &'a [String],
) -> Result<&'a str, TaskSaveDirError> {
    if !is_safe_absolute_path(save_dir) {
        return Err(TaskSaveDirError::Unauthorized);
    }

    let mut matched_root: Option<&str> = None;
    for path in accessible_paths {
        let path = path.trim();
        if !is_safe_absolute_path(path) || !is_same_or_descendant_path(save_dir, path) {
            continue;
        }
        if matched_root == Some(path) {
            return Err(TaskSaveDirError::Unauthorized);
        }
        if matched_root.map_or(true, |root| path.len() > root.len()) {
            matched_root = Some(path);
        }
    }

    matched_root.ok_or(TaskSaveDirError::Unauthorized)
}

fn is_safe_absolute_path(path: &str) -> bool {
    path.starts_with('/')
        && path != "/"
        && !path.contains(['\0', '\\'])
        && path
            .split('/')
            .skip(1)
            .all(|component| !component.is_empty() && component != "." && component != "..")
}

fn is_same_or_descendant_path(path: &str, root: &str) -> bool {
    path == root
        || path
            .strip_prefix(root)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn ensure_safe_save_dir(save_dir: &str, authorized_root: &str) -> Result<(), TaskSaveDirError> {
    let root = Path::new(authorized_root);
    let root_metadata = fs::symlink_metadata(root)
        .map_err(|error| TaskSaveDirError::PrepareFailed(format!("无法读取授权目录：{error}")))?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(TaskSaveDirError::PrepareFailed(
            "授权目录不存在或不是普通目录".to_string(),
        ));
    }

    let canonical_root = root
        .canonicalize()
        .map_err(|error| TaskSaveDirError::PrepareFailed(format!("无法解析授权目录：{error}")))?;
    let relative_path = save_dir
        .strip_prefix(authorized_root)
        .and_then(|path| path.strip_prefix('/'));
    let mut current = canonical_root.clone();
    if let Some(relative_path) = relative_path {
        for component in relative_path.split('/') {
            current.push(component);
            ensure_physical_directory(&current)?;
        }
    }

    let canonical_save_dir = current
        .canonicalize()
        .map_err(|error| TaskSaveDirError::PrepareFailed(format!("无法解析保存目录：{error}")))?;
    if !canonical_save_dir.starts_with(&canonical_root) {
        return Err(TaskSaveDirError::PrepareFailed(
            "保存目录解析后越出授权目录".to_string(),
        ));
    }
    Ok(())
}

fn ensure_physical_directory(path: &Path) -> Result<(), TaskSaveDirError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path).map_err(|error| {
                TaskSaveDirError::PrepareFailed(format!("无法创建保存目录：{error}"))
            })?;
            fs::symlink_metadata(path).map_err(|error| {
                TaskSaveDirError::PrepareFailed(format!("无法读取新建保存目录：{error}"))
            })?
        }
        Err(error) => {
            return Err(TaskSaveDirError::PrepareFailed(format!(
                "无法读取保存目录：{error}"
            )));
        }
    };

    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(TaskSaveDirError::PrepareFailed(
            "保存目录包含符号链接或同名文件".to_string(),
        ));
    }
    Ok(())
}

pub fn normalize_paths(paths: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for path in paths {
        let path = path.trim();
        if !path.is_empty() && !normalized.iter().any(|item| item == path) {
            normalized.push(path.to_string());
        }
    }
    normalized
}

fn is_data_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    normalized.ends_with("/data") || normalized == "data" || normalized.contains("/data/")
}

#[cfg(test)]
mod tests;
