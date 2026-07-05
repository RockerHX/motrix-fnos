use serde::{Deserialize, Serialize};

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
    matches!(
        task.status,
        DownloadTaskStatus::Pending | DownloadTaskStatus::Active
    )
}

pub fn should_force_pause_task_on_startup(task: &DownloadTask) -> bool {
    should_pause_task_on_exit(task)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTask {
    pub id: u64,
    pub url: String,
    pub file_name: String,
    pub save_dir: String,
    pub gid: Option<String>,
    pub status: DownloadTaskStatus,
    pub total_length: u64,
    pub completed_length: u64,
    pub download_speed: u64,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub file_path: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDownloadTaskRequest {
    pub url: String,
    pub file_name: Option<String>,
    pub save_dir: Option<String>,
    #[serde(default)]
    pub aria2_options: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedDownloadTask {
    pub url: String,
    pub file_name: String,
    pub save_dir: String,
    pub aria2_options: serde_json::Map<String, serde_json::Value>,
}

