use crate::database::tasks::{
    delete_download_task_record, persist_download_task_state, persist_download_task_states,
    upsert_download_task,
};
use crate::tasks::DownloadTask;
use axum::async_trait;
use sqlx::SqlitePool;

#[async_trait]
pub trait TaskRepository: Send + Sync {
    async fn upsert_task(&self, task: &DownloadTask) -> Result<(), String>;
    async fn persist_task_state(&self, task: &DownloadTask) -> Result<(), String>;
    async fn persist_task_states(&self, tasks: &[DownloadTask]) -> Result<(), String>;
    async fn delete_task_record(&self, task_id: u64) -> Result<bool, String>;
}

#[derive(Clone, Copy)]
pub struct SqliteTaskRepository<'a> {
    pool: &'a SqlitePool,
}

impl<'a> SqliteTaskRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TaskRepository for SqliteTaskRepository<'_> {
    async fn upsert_task(&self, task: &DownloadTask) -> Result<(), String> {
        upsert_download_task(self.pool, task).await
    }

    async fn persist_task_state(&self, task: &DownloadTask) -> Result<(), String> {
        persist_download_task_state(self.pool, task).await
    }

    async fn persist_task_states(&self, tasks: &[DownloadTask]) -> Result<(), String> {
        persist_download_task_states(self.pool, tasks).await
    }

    async fn delete_task_record(&self, task_id: u64) -> Result<bool, String> {
        delete_download_task_record(self.pool, task_id).await
    }
}
