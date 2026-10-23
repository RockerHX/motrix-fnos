use crate::database::settings::{
    get_download_proxy_config, set_download_proxy_runtime_apply_status, BtRuntimeApplyStatus,
    StoredDownloadProxyConfig,
};
use crate::database::task_operations::{
    begin_task_operation, list_unfinished_task_operations,
    list_unfinished_task_operations_for_engine, update_task_operation,
};
use crate::database::tasks::{
    delete_download_task_record_with_operation, persist_download_task_state,
    persist_download_task_state_with_operation, persist_download_task_states, upsert_download_task,
};
use crate::tasks::{DownloadTask, TaskOperation};
use axum::async_trait;
use sqlx::SqlitePool;

#[async_trait]
pub trait TaskRepository: Send + Sync {
    async fn get_download_proxy_config(&self) -> Result<Option<StoredDownloadProxyConfig>, String>;
    async fn set_bt_proxy_runtime_apply_status(
        &self,
        status: BtRuntimeApplyStatus,
    ) -> Result<(), String>;
    async fn upsert_task(&self, task: &DownloadTask) -> Result<(), String>;
    async fn persist_task_state(&self, task: &DownloadTask) -> Result<(), String>;
    async fn persist_task_states(&self, tasks: &[DownloadTask]) -> Result<(), String>;
    async fn begin_operation(&self, operation: &TaskOperation) -> Result<(), String>;
    async fn update_operation(&self, operation: &TaskOperation) -> Result<(), String>;
    async fn persist_task_state_with_operation(
        &self,
        task: &DownloadTask,
        operation: &TaskOperation,
    ) -> Result<(), String>;
    async fn list_unfinished_operations(&self) -> Result<Vec<TaskOperation>, String>;
    async fn delete_task_record_with_operation(
        &self,
        task_id: u64,
        operation: &TaskOperation,
    ) -> Result<bool, String>;
}

#[derive(Clone, Copy)]
pub struct SqliteTaskRepository<'a> {
    pool: &'a SqlitePool,
    generation: Option<crate::config::aria2::EngineGeneration>,
}

impl<'a> SqliteTaskRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self {
            pool,
            generation: None,
        }
    }

    pub(crate) fn for_engine(
        pool: &'a SqlitePool,
        generation: crate::config::aria2::EngineGeneration,
    ) -> Self {
        Self {
            pool,
            generation: Some(generation),
        }
    }
}

#[async_trait]
impl TaskRepository for SqliteTaskRepository<'_> {
    async fn get_download_proxy_config(&self) -> Result<Option<StoredDownloadProxyConfig>, String> {
        get_download_proxy_config(self.pool).await
    }

    async fn set_bt_proxy_runtime_apply_status(
        &self,
        status: BtRuntimeApplyStatus,
    ) -> Result<(), String> {
        set_download_proxy_runtime_apply_status(self.pool, status).await
    }

    async fn upsert_task(&self, task: &DownloadTask) -> Result<(), String> {
        upsert_download_task(self.pool, task).await
    }

    async fn persist_task_state(&self, task: &DownloadTask) -> Result<(), String> {
        persist_download_task_state(self.pool, task).await
    }

    async fn persist_task_states(&self, tasks: &[DownloadTask]) -> Result<(), String> {
        persist_download_task_states(self.pool, tasks).await
    }

    async fn begin_operation(&self, operation: &TaskOperation) -> Result<(), String> {
        begin_task_operation(self.pool, operation).await
    }

    async fn update_operation(&self, operation: &TaskOperation) -> Result<(), String> {
        update_task_operation(self.pool, operation).await
    }

    async fn persist_task_state_with_operation(
        &self,
        task: &DownloadTask,
        operation: &TaskOperation,
    ) -> Result<(), String> {
        persist_download_task_state_with_operation(self.pool, task, operation).await
    }

    async fn list_unfinished_operations(&self) -> Result<Vec<TaskOperation>, String> {
        match self.generation {
            Some(generation) => {
                list_unfinished_task_operations_for_engine(self.pool, generation).await
            }
            None => list_unfinished_task_operations(self.pool).await,
        }
    }

    async fn delete_task_record_with_operation(
        &self,
        task_id: u64,
        operation: &TaskOperation,
    ) -> Result<bool, String> {
        delete_download_task_record_with_operation(self.pool, task_id, operation).await
    }
}
