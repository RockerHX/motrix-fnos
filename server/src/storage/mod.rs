use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskSaveDirError {
    Required,
    NoAccessiblePaths,
    Unauthorized,
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
    if accessible_paths.iter().any(|path| path == save_dir) {
        return Ok(());
    }
    Err(TaskSaveDirError::Unauthorized)
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

pub(crate) fn is_authorized_path(
    path: &Path,
    accessible_paths: &[String],
    allow_descendant: bool,
) -> bool {
    let Some(path) = canonicalize_path_or_parent(path) else {
        return false;
    };
    accessible_paths.iter().any(|root| {
        let Some(root) = canonicalize_path_or_parent(Path::new(root)) else {
            return false;
        };
        if allow_descendant {
            path.starts_with(root)
        } else {
            path == root
        }
    })
}

pub(crate) fn is_authorized_directory_path(
    path: &Path,
    accessible_paths: &[String],
    allow_descendant: bool,
) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => return false,
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return false,
    }
    is_authorized_path(path, accessible_paths, allow_descendant)
}

pub(crate) fn is_safe_path_syntax(path: &Path) -> bool {
    let Some(value) = path.to_str() else {
        return false;
    };
    path.is_absolute()
        && !value.contains(['\0', '\\'])
        && !value
            .split('/')
            .any(|segment| matches!(segment, "." | ".."))
}

pub(crate) fn canonicalize_path_or_parent(path: &Path) -> Option<PathBuf> {
    if !is_safe_path_syntax(path) {
        return None;
    }
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return None;
            }
            return path.canonicalize().ok();
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return None,
    }

    let mut suffix = Vec::new();
    let mut current = path;
    loop {
        match std::fs::symlink_metadata(current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return None;
                }
                let mut canonical = current.canonicalize().ok()?;
                for component in suffix.iter().rev() {
                    canonical.push(component);
                }
                return Some(canonical);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return None,
        }
        suffix.push(current.file_name()?.to_os_string());
        current = current.parent()?;
    }
}

fn is_data_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    normalized.ends_with("/data") || normalized == "data" || normalized.contains("/data/")
}

#[cfg(test)]
mod tests;
