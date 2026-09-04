use super::{
    classify_aria2_ready_error, classify_task_error, ensure_authorized_save_dir,
    validate_authorized_save_dir,
};
use crate::api::build_task_service;
use crate::api::error::ApiError;
use crate::app::HttpAppState;
use crate::runtime::{broadcast_tasks_snapshot, ensure_aria2_ready, ReadyAria2};
use crate::tasks::service::TaskService;
use crate::tasks::{DownloadTask, PublicDownloadTask};

pub(super) struct TaskMutationContext<'a> {
    pub(super) state: &'a HttpAppState,
    pub(super) service: TaskService<'a>,
    pub(super) config: ReadyAria2,
}

impl<'a> TaskMutationContext<'a> {
    pub(super) async fn prepare(state: &'a HttpAppState) -> Result<Self, ApiError> {
        Self::prepare_inner(state, None).await
    }

    pub(super) async fn prepare_for_create(
        state: &'a HttpAppState,
        save_dir: Option<&str>,
    ) -> Result<Self, ApiError> {
        Self::prepare_inner(state, Some(save_dir)).await
    }

    async fn prepare_inner(
        state: &'a HttpAppState,
        save_dir: Option<Option<&str>>,
    ) -> Result<Self, ApiError> {
        let service = build_task_service(state);
        service.ensure_not_exiting().map_err(classify_task_error)?;
        if let Some(save_dir) = save_dir {
            validate_authorized_save_dir(state, save_dir)?;
        }
        let config = ensure_aria2_ready(state)
            .await
            .map_err(classify_aria2_ready_error)?;
        if let Some(save_dir) = save_dir {
            ensure_authorized_save_dir(state, save_dir)?;
        }
        Ok(Self {
            state,
            service,
            config,
        })
    }

    pub(super) fn finish(
        &self,
        task: DownloadTask,
    ) -> Result<axum::Json<PublicDownloadTask>, ApiError> {
        self.broadcast_snapshot()?;
        Ok(axum::Json(task.into()))
    }

    pub(super) fn broadcast_snapshot(&self) -> Result<(), ApiError> {
        broadcast_tasks_snapshot(self.state)
            .map_err(|error| ApiError::internal("tasks_snapshot_broadcast_failed", error))
    }
}
