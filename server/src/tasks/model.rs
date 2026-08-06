use serde::{Deserialize, Serialize};
use std::fmt;

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
    pub source_type: DownloadTaskSourceType,
    pub file_name: String,
    pub save_dir: String,
    /// 应用为 BT 任务创建并拥有的外层目录；与可变化的任务显示名无关。
    pub owned_task_dir: Option<String>,
    pub category: String,
    pub gid: Option<String>,
    pub status: DownloadTaskStatus,
    pub total_length: u64,
    pub completed_length: u64,
    pub download_speed: u64,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub file_path: Option<String>,
    #[serde(default)]
    pub use_proxy: bool,
    #[serde(skip, default)]
    pub proxy_binding: TaskProxyBinding,
    #[serde(skip, default)]
    pub metadata_torrent_path: Option<String>,
    #[serde(skip, default)]
    pub files_deleted: bool,
    #[serde(skip, default)]
    pub selected_file_indexes: Vec<u32>,
    pub confirmation_required: bool,
    pub files: Vec<DownloadTaskFile>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Clone, PartialEq, Eq)]
pub struct SensitiveProxyUrl(String);

impl SensitiveProxyUrl {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SensitiveProxyUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SensitiveProxyUrl([REDACTED])")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TaskProxySource {
    #[default]
    Profile,
    Override,
}

impl TaskProxySource {
    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::Profile => "profile",
            Self::Override => "override",
        }
    }

    pub fn from_storage_value(value: &str) -> Self {
        match value {
            "override" => Self::Override,
            _ => Self::Profile,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Default)]
pub struct TaskProxyBinding {
    source: TaskProxySource,
    effective_proxy_url: Option<SensitiveProxyUrl>,
}

impl TaskProxyBinding {
    pub fn profile(proxy_url: Option<String>) -> Self {
        Self {
            source: TaskProxySource::Profile,
            effective_proxy_url: proxy_url.map(SensitiveProxyUrl::new),
        }
    }

    pub fn override_url(proxy_url: String) -> Self {
        Self {
            source: TaskProxySource::Override,
            effective_proxy_url: Some(SensitiveProxyUrl::new(proxy_url)),
        }
    }

    pub fn from_persisted(source: TaskProxySource, proxy_url: Option<String>) -> Self {
        Self {
            source,
            effective_proxy_url: proxy_url.map(SensitiveProxyUrl::new),
        }
    }

    pub fn source(&self) -> TaskProxySource {
        self.source
    }

    pub fn effective_proxy_url(&self) -> Option<&str> {
        self.effective_proxy_url
            .as_ref()
            .map(SensitiveProxyUrl::expose)
    }
}

impl fmt::Debug for TaskProxyBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TaskProxyBinding")
            .field("source", &self.source)
            .field(
                "effective_proxy_url",
                &self.effective_proxy_url.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum DownloadTaskSourceType {
    #[default]
    Url,
    Torrent,
    Magnet,
}

impl DownloadTaskSourceType {
    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::Url => "url",
            Self::Torrent => "torrent",
            Self::Magnet => "magnet",
        }
    }

    pub fn from_storage_value(value: &str) -> Self {
        match value {
            "torrent" => Self::Torrent,
            "magnet" => Self::Magnet,
            _ => Self::Url,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum DownloadTaskStartMode {
    #[default]
    Now,
    Paused,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskAdvancedOptions {
    pub connections: Option<u32>,
    pub download_limit_kb: Option<u64>,
    pub use_proxy: Option<bool>,
    pub proxy: Option<String>,
}

impl fmt::Debug for CreateTaskAdvancedOptions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CreateTaskAdvancedOptions")
            .field("connections", &self.connections)
            .field("download_limit_kb", &self.download_limit_kb)
            .field("use_proxy", &self.use_proxy)
            .field("proxy", &self.proxy.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

#[derive(Deserialize)]
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

impl fmt::Debug for CreateDownloadTaskRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CreateDownloadTaskRequest")
            .field("url", &self.url)
            .field("file_name", &self.file_name)
            .field("save_dir", &self.save_dir)
            .field("source_type", &self.source_type)
            .field("start_mode", &self.start_mode)
            .field("category", &self.category)
            .field("advanced_options", &self.advanced_options)
            .field(
                "aria2_option_keys",
                &self.aria2_options.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

pub struct CreateTorrentDownloadTaskRequest {
    pub torrent_file_name: String,
    pub torrent_data: Vec<u8>,
    pub save_dir: String,
    pub start_mode: DownloadTaskStartMode,
    pub category: Option<String>,
    pub advanced_options: CreateTaskAdvancedOptions,
}

impl fmt::Debug for CreateTorrentDownloadTaskRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CreateTorrentDownloadTaskRequest")
            .field("torrent_file_name", &self.torrent_file_name)
            .field("torrent_data_len", &self.torrent_data.len())
            .field("save_dir", &self.save_dir)
            .field("start_mode", &self.start_mode)
            .field("category", &self.category)
            .field("advanced_options", &self.advanced_options)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedDownloadTask {
    pub url: String,
    pub file_name: String,
    pub output_file_name: Option<String>,
    pub save_dir: String,
    pub aria2_save_dir: Option<String>,
    pub category: String,
    pub source_type: DownloadTaskSourceType,
    pub start_mode: DownloadTaskStartMode,
    pub advanced_options: CreateTaskAdvancedOptions,
    pub aria2_options: serde_json::Map<String, serde_json::Value>,
    pub use_proxy: bool,
    pub proxy_binding: TaskProxyBinding,
}
