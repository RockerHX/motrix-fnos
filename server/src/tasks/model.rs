use serde::{Deserialize, Serialize};

pub const DEFAULT_TASK_CATEGORY: &str = "默认";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DownloadTaskStatus {
    Pending,
    Active,
    Paused,
    Complete,
    Error,
    Removed,
}

impl DownloadTaskStatus {
    pub fn as_storage_value(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Complete => "complete",
            Self::Error => "error",
            Self::Removed => "removed",
        }
    }

    pub fn from_storage_value(value: &str) -> Self {
        match value {
            "pending" => Self::Pending,
            "active" => Self::Active,
            "paused" => Self::Paused,
            "complete" => Self::Complete,
            "error" => Self::Error,
            "removed" => Self::Removed,
            _ => Self::Pending,
        }
    }
}

pub fn should_pause_task_on_exit(task: &DownloadTask) -> bool {
    if task.confirmation_required {
        return false;
    }

    matches!(
        task.status,
        DownloadTaskStatus::Pending | DownloadTaskStatus::Active
    )
}

pub fn should_force_pause_task_on_startup(task: &DownloadTask) -> bool {
    should_pause_task_on_exit(task)
}

pub fn is_pending_magnet_metadata_task(task: &DownloadTask) -> bool {
    task.url.to_ascii_lowercase().starts_with("magnet:?")
        && !task.confirmation_required
        && task
            .metadata_torrent_path
            .as_deref()
            .map(|path| path.trim().is_empty())
            .unwrap_or(true)
        && task
            .file_path
            .as_deref()
            .map(|path| path.trim().is_empty())
            .unwrap_or(true)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTaskFile {
    pub index: u32,
    pub path: String,
    pub name: String,
    pub length: u64,
    pub completed_length: u64,
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTask {
    pub id: u64,
    pub url: String,
    pub file_name: String,
    pub save_dir: String,
    pub category: String,
    pub gid: Option<String>,
    pub status: DownloadTaskStatus,
    pub total_length: u64,
    pub completed_length: u64,
    pub download_speed: u64,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub file_path: Option<String>,
    pub metadata_torrent_path: Option<String>,
    pub confirmation_required: bool,
    pub files: Vec<DownloadTaskFile>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DownloadTaskSourceType {
    Url,
    Magnet,
}

impl Default for DownloadTaskSourceType {
    fn default() -> Self {
        Self::Url
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DownloadTaskStartMode {
    Now,
    Paused,
}

impl Default for DownloadTaskStartMode {
    fn default() -> Self {
        Self::Now
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskAdvancedOptions {
    pub connections: Option<u32>,
    pub download_limit_kb: Option<u64>,
    pub proxy: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDownloadTaskRequest {
    pub url: String,
    pub file_name: Option<String>,
    pub save_dir: Option<String>,
    #[serde(default)]
    pub source_type: DownloadTaskSourceType,
    #[serde(default)]
    pub start_mode: DownloadTaskStartMode,
    pub category: Option<String>,
    #[serde(default)]
    pub advanced_options: CreateTaskAdvancedOptions,
    #[serde(default)]
    pub aria2_options: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug)]
pub struct CreateTorrentDownloadTaskRequest {
    pub torrent_file_name: String,
    pub torrent_data: Vec<u8>,
    pub save_dir: String,
    pub start_mode: DownloadTaskStartMode,
    pub category: Option<String>,
    pub advanced_options: CreateTaskAdvancedOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedDownloadTask {
    pub url: String,
    pub file_name: String,
    pub save_dir: String,
    pub aria2_save_dir: Option<String>,
    pub category: String,
    pub source_type: DownloadTaskSourceType,
    pub start_mode: DownloadTaskStartMode,
    pub advanced_options: CreateTaskAdvancedOptions,
    pub aria2_options: serde_json::Map<String, serde_json::Value>,
}
