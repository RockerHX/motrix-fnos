use super::{DownloadTask, DownloadTaskFile, DownloadTaskSourceType, DownloadTaskStatus};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PublicDownloadTask {
    pub id: u64,
    pub engine_generation: crate::config::aria2::EngineGeneration,
    pub url: String,
    pub source_type: DownloadTaskSourceType,
    pub file_name: String,
    pub save_dir: String,
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
    pub use_proxy: bool,
    pub files_deleted: bool,
    #[serde(default)]
    pub cleanup_state: String,
    pub confirmation_required: bool,
    pub files: Vec<DownloadTaskFile>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl From<DownloadTask> for PublicDownloadTask {
    fn from(task: DownloadTask) -> Self {
        let DownloadTask {
            id,
            engine_generation,
            url,
            source_type,
            file_name,
            save_dir,
            owned_task_dir,
            category,
            gid,
            status,
            removed_from_status: _,
            total_length,
            completed_length,
            download_speed,
            error_code,
            error_message,
            file_path,
            use_proxy,
            proxy_binding: _,
            metadata_torrent_path: _,
            files_deleted,
            selected_file_indexes: _,
            confirmation_required,
            files,
            created_at,
            updated_at,
        } = task;

        Self {
            id,
            engine_generation,
            url,
            source_type,
            file_name,
            save_dir,
            owned_task_dir,
            category,
            gid,
            status,
            total_length,
            completed_length,
            download_speed,
            error_code,
            error_message,
            file_path,
            use_proxy,
            files_deleted,
            cleanup_state: "none".to_string(),
            confirmation_required,
            files,
            created_at,
            updated_at,
        }
    }
}
