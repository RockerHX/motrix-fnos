use super::{
    classify_aria2_ready_error, classify_task_error, ensure_authorized_save_dir,
    validate_authorized_save_dir,
};
use crate::api::build_task_service;
use crate::api::error::ApiError;
use crate::app::HttpAppState;
use crate::config::aria2::Aria2Config;
use crate::runtime::{broadcast_tasks_snapshot, ensure_aria2_ready, ReadyAria2};
use crate::tasks::service::TaskService;
use crate::tasks::{DownloadTask, DownloadTaskStatus, PublicDownloadTask};

pub(super) struct TaskMutationContext<'a> {
    pub(super) state: &'a HttpAppState,
    pub(super) service: TaskService<'a>,
    pub(super) config: Aria2Config,
    _activity: Option<ReadyAria2>,
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

    pub(super) async fn prepare_for_task(
        state: &'a HttpAppState,
        task_id: u64,
    ) -> Result<Self, ApiError> {
        let service = build_task_service(state);
        service.ensure_not_exiting().map_err(classify_task_error)?;
        service
            .ensure_task_authorized(task_id)
            .map_err(classify_task_error)?;
        let config = ensure_aria2_ready(state)
            .await
            .map_err(classify_aria2_ready_error)?;
        Ok(Self {
            state,
            service,
            config: (*config).clone(),
            _activity: Some(config),
        })
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
            config: (*config).clone(),
            _activity: Some(config),
        })
    }

    pub(super) async fn prepare_for_recycle(
        state: &'a HttpAppState,
        task_id: u64,
        restart_from_zero: bool,
    ) -> Result<Self, ApiError> {
        let service = build_task_service(state);
        service.ensure_not_exiting().map_err(classify_task_error)?;
        service
            .ensure_file_cleanup_not_pending(task_id)
            .await
            .map_err(classify_task_error)?;
        let task = service
            .get_download_task(task_id)
            .map_err(classify_task_error)?
            .ok_or_else(|| ApiError::not_found("task_not_found", "下载任务不存在"))?;
        if task.status != DownloadTaskStatus::Complete
            && (task.removed_from_status != Some(DownloadTaskStatus::Complete) || restart_from_zero)
        {
            return Self::prepare_for_task(state, task_id).await;
        }
        let mut config = state.aria2_config();
        if task.engine_generation == crate::config::aria2::EngineGeneration::NativeV2 {
            config.state_dir = Some(
                crate::state::native_aria2_runtime_paths(&state.core.app_data_dir)
                    .state_dir
                    .display()
                    .to_string(),
            );
        }
        Ok(Self {
            state,
            service,
            config,
            _activity: None,
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
