use super::AccessiblePathsResponse;
use crate::fnos::{FnosApiClient, FnosApiError, SharedAccessibleFolders};
use std::collections::HashSet;
use std::fmt;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SNAPSHOT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccessiblePathsRefreshResult {
    pub(crate) paths: Vec<String>,
    pub(crate) http_status: u16,
    pub(crate) business_code: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AccessiblePathsRefreshError {
    Fnos(FnosApiError),
    InvalidPaths,
    Persist,
}

impl fmt::Display for AccessiblePathsRefreshError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fnos(error) => error.fmt(formatter),
            Self::InvalidPaths => formatter.write_str("fnOS 返回的共享授权目录格式无效"),
            Self::Persist => formatter.write_str("保存共享授权目录快照失败"),
        }
    }
}

impl From<FnosApiError> for AccessiblePathsRefreshError {
    fn from(error: FnosApiError) -> Self {
        Self::Fnos(error)
    }
}

pub(crate) async fn refresh_accessible_paths_from_fnos(
    client: &FnosApiClient,
    snapshot_path: &Path,
) -> Result<AccessiblePathsRefreshResult, AccessiblePathsRefreshError> {
    let folders = client.query_shared_accessible_folders().await;
    validate_and_persist_query_result(snapshot_path, folders)
}

fn validate_and_persist_query_result(
    snapshot_path: &Path,
    folders: Result<SharedAccessibleFolders, FnosApiError>,
) -> Result<AccessiblePathsRefreshResult, AccessiblePathsRefreshError> {
    let folders = folders?;
    let paths = validate_official_paths(folders.paths)?;
    persist_accessible_paths_atomic(snapshot_path, &paths)?;
    Ok(AccessiblePathsRefreshResult {
        paths,
        http_status: folders.http_status,
        business_code: folders.business_code,
    })
}

fn validate_official_paths(paths: Vec<String>) -> Result<Vec<String>, AccessiblePathsRefreshError> {
    let mut seen = HashSet::new();
    let mut validated = Vec::with_capacity(paths.len());
    for value in paths {
        let path = Path::new(&value);
        let invalid_segment = value
            .split('/')
            .any(|segment| segment == "." || segment == "..");
        if value.is_empty()
            || value.chars().any(char::is_whitespace)
            || value.contains(['\0', '\\'])
            || !path.is_absolute()
            || value.trim_matches('/').is_empty()
            || invalid_segment
        {
            return Err(AccessiblePathsRefreshError::InvalidPaths);
        }
        if seen.insert(value.clone()) {
            validated.push(value);
        }
    }
    Ok(validated)
}

fn persist_accessible_paths_atomic(
    snapshot_path: &Path,
    paths: &[String],
) -> Result<(), AccessiblePathsRefreshError> {
    let parent = snapshot_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or(AccessiblePathsRefreshError::Persist)?;
    let file_name = snapshot_path
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or(AccessiblePathsRefreshError::Persist)?;
    let payload = serde_json::to_vec(&AccessiblePathsResponse {
        paths: paths.to_vec(),
    })
    .map_err(|_| AccessiblePathsRefreshError::Persist)?;
    let (temp_path, mut file) = create_snapshot_temp_file(parent, file_name)?;

    let result = (|| {
        file.write_all(&payload)
            .map_err(|_| AccessiblePathsRefreshError::Persist)?;
        file.sync_all()
            .map_err(|_| AccessiblePathsRefreshError::Persist)?;
        drop(file);
        std::fs::rename(&temp_path, snapshot_path).map_err(|_| AccessiblePathsRefreshError::Persist)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    result
}

fn create_snapshot_temp_file(
    parent: &Path,
    file_name: &str,
) -> Result<(PathBuf, std::fs::File), AccessiblePathsRefreshError> {
    for _ in 0..16 {
        let temp_path = parent.join(format!(
            ".{file_name}.tmp-{}-{}",
            std::process::id(),
            SNAPSHOT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => return Ok((temp_path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(AccessiblePathsRefreshError::Persist),
        }
    }
    Err(AccessiblePathsRefreshError::Persist)
}

#[cfg(test)]
mod tests;
