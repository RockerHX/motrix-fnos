use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const DATA_ACCESSIBLE_PATHS_ENV: &str = "TRIM_DATA_ACCESSIBLE_PATHS";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccessiblePathsResponse {
    pub paths: Vec<String>,
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

    Ok(normalize_paths(
        std::env::var(DATA_ACCESSIBLE_PATHS_ENV)
            .ok()
            .map(|value| value.split(':').map(str::to_string).collect())
            .unwrap_or_default(),
    ))
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

fn is_data_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    normalized.ends_with("/data") || normalized == "data" || normalized.contains("/data/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_download_dir_prefers_data_authorized_path() {
        let paths = vec![
            "/vol1/tmp".to_string(),
            "/应用文件/motrix_fnos/data".to_string(),
            "/vol1/downloads".to_string(),
        ];

        assert_eq!(
            default_download_dir(&paths, Path::new("/fallback")),
            PathBuf::from("/应用文件/motrix_fnos/data")
        );
    }

    #[test]
    fn default_download_dir_uses_first_authorized_path_when_data_missing() {
        let paths = vec!["/vol1/downloads".to_string(), "/vol1/tmp".to_string()];

        assert_eq!(
            default_download_dir(&paths, Path::new("/fallback")),
            PathBuf::from("/vol1/downloads")
        );
    }

    #[test]
    fn default_download_dir_falls_back_to_app_data_dir_when_authorized_paths_empty() {
        assert_eq!(
            default_download_dir(&[], Path::new("/app/data")),
            PathBuf::from("/app/data")
        );
    }

    #[test]
    fn load_accessible_paths_normalizes_file_values() {
        let path = std::env::temp_dir().join(format!(
            "motrix-fnos-accessible-paths-test-{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be valid")
                .as_nanos()
        ));
        std::fs::write(
            &path,
            r#"{"paths":[" /app/data ","/app/data","","/vol1/tmp"]}"#,
        )
        .expect("accessible paths should write");

        let paths = load_accessible_paths(&path).expect("paths should load");

        assert_eq!(paths, vec!["/app/data", "/vol1/tmp"]);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn validate_default_download_dir_accepts_authorized_path() {
        let paths = vec!["/app/data".to_string()];

        assert!(validate_default_download_dir("/app/data", &paths, Path::new("/fallback")).is_ok());
    }

    #[test]
    fn validate_default_download_dir_rejects_unauthorized_path() {
        let paths = vec!["/app/data".to_string()];

        let error = validate_default_download_dir("/tmp", &paths, Path::new("/fallback"))
            .expect_err("unauthorized path should fail");

        assert_eq!(error, "默认下载目录不在已授权目录列表中");
    }

    #[test]
    fn validate_default_download_dir_allows_app_data_dir_without_authorized_paths() {
        assert!(validate_default_download_dir("/app/data", &[], Path::new("/app/data")).is_ok());
    }

    #[test]
    fn validate_task_save_dir_requires_non_empty_path() {
        assert_eq!(
            validate_task_save_dir(Some("  "), &["/downloads".to_string()]),
            Err(TaskSaveDirError::Required)
        );
    }

    #[test]
    fn validate_task_save_dir_requires_authorized_paths() {
        assert_eq!(
            validate_task_save_dir(Some("/downloads"), &[]),
            Err(TaskSaveDirError::NoAccessiblePaths)
        );
    }

    #[test]
    fn validate_task_save_dir_accepts_only_exact_authorized_path() {
        let paths = vec!["/downloads".to_string()];
        assert!(validate_task_save_dir(Some("/downloads"), &paths).is_ok());
        assert_eq!(
            validate_task_save_dir(Some("/downloads/movies"), &paths),
            Err(TaskSaveDirError::Unauthorized)
        );
    }
}
