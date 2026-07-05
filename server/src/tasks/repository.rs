use crate::database::tasks::{
    delete_download_task_record, persist_download_task_state, persist_download_task_states,
    upsert_download_task,
};
use crate::tasks::DownloadTask;
use sqlx::SqlitePool;

#[derive(Clone, Copy)]
pub struct TaskRepository<'a> {
    pool: &'a SqlitePool,
}

impl<'a> TaskRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert_task(&self, task: &DownloadTask) -> Result<(), String> {
        upsert_download_task(self.pool, task).await
    }

    pub async fn persist_task_state(&self, task: &DownloadTask) -> Result<(), String> {
        persist_download_task_state(self.pool, task).await
    }

    pub async fn persist_task_states(&self, tasks: &[DownloadTask]) -> Result<(), String> {
        persist_download_task_states(self.pool, tasks).await
    }

    pub async fn delete_task_record(&self, task_id: u64) -> Result<bool, String> {
        delete_download_task_record(self.pool, task_id).await
    }
}
