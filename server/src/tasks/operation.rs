use super::DownloadTask;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub const FILE_CLEANUP_PENDING_PHASE: &str = "file_cleanup_pending";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskOperationType {
    Create,
    Confirm,
    Redownload,
    Pause,
    Resume,
    Delete,
    Restore,
    PermanentDelete,
    Proxy,
}

impl TaskOperationType {
    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Confirm => "confirm",
            Self::Redownload => "redownload",
            Self::Pause => "pause",
            Self::Resume => "resume",
            Self::Delete => "delete",
            Self::Restore => "restore",
            Self::PermanentDelete => "permanent_delete",
            Self::Proxy => "proxy",
        }
    }

    pub fn from_storage_value(value: &str) -> Result<Self, String> {
        match value {
            "create" => Ok(Self::Create),
            "confirm" => Ok(Self::Confirm),
            "redownload" => Ok(Self::Redownload),
            "pause" => Ok(Self::Pause),
            "resume" => Ok(Self::Resume),
            "delete" => Ok(Self::Delete),
            "restore" => Ok(Self::Restore),
            "permanent_delete" => Ok(Self::PermanentDelete),
            "proxy" => Ok(Self::Proxy),
            _ => Err(format!("未知任务操作类型：{}", value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskOperationStatus {
    InProgress,
    Completed,
    Failed,
    ManualReview,
}

impl TaskOperationStatus {
    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::ManualReview => "manual_review",
        }
    }

    pub fn from_storage_value(value: &str) -> Result<Self, String> {
        match value {
            "in_progress" => Ok(Self::InProgress),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "manual_review" => Ok(Self::ManualReview),
            _ => Err(format!("未知任务操作状态：{}", value)),
        }
    }

    pub fn is_unfinished(self) -> bool {
        matches!(self, Self::InProgress | Self::ManualReview)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskOperationContext {
    pub old_gid: Option<String>,
    pub new_gid: Option<String>,
    #[serde(default)]
    pub aria2_request: Option<Aria2TaskRequest>,
    #[serde(default)]
    pub critical_paths: Vec<String>,
    #[serde(default)]
    pub file_cleanup_paths: Vec<String>,
    #[serde(default)]
    pub completed_side_effects: Vec<String>,
    #[serde(default)]
    pub proxy_enabled: Option<bool>,
    pub task_snapshot: Option<DownloadTask>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Aria2TaskRequest {
    pub request_id: String,
    pub source_url: String,
    pub save_dir: String,
    pub file_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskOperation {
    pub id: String,
    pub task_id: u64,
    pub operation_type: TaskOperationType,
    pub phase: String,
    pub context: TaskOperationContext,
    pub error_message: Option<String>,
    pub status: TaskOperationStatus,
    pub created_at: u64,
    pub updated_at: u64,
}

impl TaskOperation {
    pub fn new(
        task_id: u64,
        operation_type: TaskOperationType,
        phase: impl Into<String>,
        context: TaskOperationContext,
    ) -> Self {
        let now = current_timestamp_ms();
        Self {
            id: new_operation_id(),
            task_id,
            operation_type,
            phase: phase.into(),
            context,
            error_message: None,
            status: TaskOperationStatus::InProgress,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_id(
        id: impl Into<String>,
        task_id: u64,
        operation_type: TaskOperationType,
        phase: impl Into<String>,
        context: TaskOperationContext,
    ) -> Self {
        let mut operation = Self::new(task_id, operation_type, phase, context);
        operation.id = id.into();
        operation
    }

    pub fn update_phase(&mut self, phase: impl Into<String>, context: TaskOperationContext) {
        self.phase = phase.into();
        self.context = context;
        self.updated_at = current_timestamp_ms();
    }

    pub fn complete(&mut self, phase: impl Into<String>) {
        self.phase = phase.into();
        self.status = TaskOperationStatus::Completed;
        self.error_message = None;
        self.updated_at = current_timestamp_ms();
    }

    pub fn fail(&mut self, phase: impl Into<String>, message: impl Into<String>) {
        self.phase = phase.into();
        self.status = TaskOperationStatus::Failed;
        self.error_message = Some(message.into());
        self.updated_at = current_timestamp_ms();
    }

    pub fn require_manual_review(&mut self, phase: impl Into<String>, message: impl Into<String>) {
        self.phase = phase.into();
        self.status = TaskOperationStatus::ManualReview;
        self.error_message = Some(message.into());
        self.updated_at = current_timestamp_ms();
    }

    pub fn is_file_cleanup_pending(&self) -> bool {
        self.operation_type == TaskOperationType::Delete
            && self.phase == FILE_CLEANUP_PENDING_PHASE
            && self.status.is_unfinished()
    }

    pub fn retain_file_cleanup_pending(&mut self, message: impl Into<String>) {
        self.phase = FILE_CLEANUP_PENDING_PHASE.to_string();
        self.status = TaskOperationStatus::InProgress;
        self.error_message = Some(message.into());
        self.updated_at = current_timestamp_ms();
    }
}

fn new_operation_id() -> String {
    let mut bytes = [0_u8; 16];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}
