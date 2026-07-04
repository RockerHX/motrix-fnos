use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const DATA_ACCESSIBLE_PATHS_ENV: &str = "TRIM_DATA_ACCESSIBLE_PATHS";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccessiblePathsResponse {
    pub paths: Vec<String>,
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
}
