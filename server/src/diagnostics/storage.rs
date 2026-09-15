use crate::state::{ARIA2_RUNTIME_DIR_NAME, ARIA2_SESSION_FILE_NAME};
use fs2::{available_space, total_space};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

const MAGNET_METADATA_DIRECTORY_NAME: &str = "magnet-metadata";

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StorageDiskUsage {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StorageFileUsage {
    pub total_bytes: u64,
    pub file_count: usize,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StorageUsageSnapshot {
    pub disk: StorageDiskUsage,
    pub aria2_session_bytes: u64,
    pub magnet_metadata: StorageFileUsage,
}

pub(crate) fn collect_storage_usage(app_data_dir: &Path) -> Result<StorageUsageSnapshot, String> {
    ensure_regular_directory(app_data_dir, "应用数据目录")?;

    let disk = StorageDiskUsage {
        total_bytes: total_space(app_data_dir)
            .map_err(|error| format!("读取应用数据所在文件系统总空间失败：{error}"))?,
        available_bytes: available_space(app_data_dir)
            .map_err(|error| format!("读取应用数据所在文件系统可用空间失败：{error}"))?,
    };
    let aria2_session_bytes = collect_aria2_session_size(app_data_dir)?;
    let magnet_metadata =
        collect_regular_file_usage(&app_data_dir.join(MAGNET_METADATA_DIRECTORY_NAME))?;

    Ok(StorageUsageSnapshot {
        disk,
        aria2_session_bytes,
        magnet_metadata,
    })
}

fn collect_aria2_session_size(app_data_dir: &Path) -> Result<u64, String> {
    let aria2_dir = app_data_dir.join(ARIA2_RUNTIME_DIR_NAME);
    if !is_regular_directory(&aria2_dir)? {
        return Ok(0);
    }

    Ok(regular_file_size(&aria2_dir.join(ARIA2_SESSION_FILE_NAME))?.unwrap_or_default())
}

fn collect_regular_file_usage(root: &Path) -> Result<StorageFileUsage, String> {
    if !is_regular_directory(root)? {
        return Ok(StorageFileUsage::default());
    }

    let mut usage = StorageFileUsage::default();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory)
            .map_err(|error| format!("读取应用私有临时目录失败：{error}"))?
        {
            let entry = entry.map_err(|error| format!("读取应用私有临时目录条目失败：{error}"))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("读取应用私有临时文件元数据失败：{error}"))?;
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                directories.push(path);
            } else if metadata.is_file() {
                usage.total_bytes = usage.total_bytes.saturating_add(metadata.len());
                usage.file_count = usage.file_count.saturating_add(1);
            }
        }
    }
    Ok(usage)
}

fn regular_file_size(path: &Path) -> Result<Option<u64>, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => Ok(None),
        Ok(metadata) => Ok(Some(metadata.len())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("读取应用私有文件元数据失败：{error}")),
    }
}

fn is_regular_directory(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(!metadata.file_type().is_symlink() && metadata.is_dir()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("读取应用私有目录元数据失败：{error}")),
    }
}

fn ensure_regular_directory(path: &Path, label: &str) -> Result<(), String> {
    if is_regular_directory(path)? {
        return Ok(());
    }
    Err(format!("{label}不是受信任的普通目录"))
}

#[cfg(test)]
mod tests;
