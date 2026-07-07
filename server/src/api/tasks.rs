use crate::api::error::ApiError;
use crate::api::extract::ApiJson;
use crate::app::HttpAppState;
use crate::runtime::{broadcast_tasks_snapshot, ensure_aria2_ready};
use crate::tasks::repository::SqliteTaskRepository;
use crate::tasks::service::{RuntimeGuard, TaskService};
use crate::tasks::{
    CreateDownloadTaskRequest, CreateTaskAdvancedOptions, DownloadTask, DownloadTaskSourceType,
    DownloadTaskStartMode,
};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub fn routes() -> Router<Arc<HttpAppState>> {
    Router::new()
        .route("/tasks", get(list_tasks).post(create_task))
        .route("/tasks/batch", post(create_batch_tasks))
        .route("/tasks/:id/pause", post(pause_task))
        .route("/tasks/:id/resume", post(resume_task))
        .route("/tasks/:id/redownload", post(redownload_task))
        .route("/tasks/:id/permanent", delete(permanently_delete_task))
        .route("/tasks/:id", delete(delete_task))
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct DeleteTaskQuery {
    delete_files: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
struct ListTasksQuery {
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBatchDownloadTasksRequest {
    urls: Vec<String>,
    save_dir: String,
    #[serde(default)]
    start_mode: DownloadTaskStartMode,
    category: Option<String>,
    #[serde(default)]
    advanced_options: CreateTaskAdvancedOptions,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBatchDownloadTasksResponse {
    created: Vec<DownloadTask>,
    failed: Vec<CreateBatchDownloadTaskFailure>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBatchDownloadTaskFailure {
    input: String,
    message: String,
}

enum ListTasksFilter {
    Visible,
    Removed,
}

impl ListTasksQuery {
    fn filter(&self) -> Result<ListTasksFilter, ApiError> {
        match self.status.as_deref().map(str::trim) {
            None | Some("") => Ok(ListTasksFilter::Visible),
            Some("removed") => Ok(ListTasksFilter::Removed),
            Some(status) => Err(ApiError::bad_request(
                "task_status_filter_invalid",
                format!("不支持的任务状态筛选：{}", status),
            )),
        }
    }
}

async fn list_tasks(
    State(state): State<Arc<HttpAppState>>,
    Query(query): Query<ListTasksQuery>,
) -> Result<Json<Vec<DownloadTask>>, ApiError> {
    let service = task_service(&state);
    if matches!(query.filter()?, ListTasksFilter::Removed) {
        let tasks = service
            .list_removed_download_tasks()
            .map_err(classify_task_error)?;
        return Ok(Json(tasks));
    }

    let config = if state.core.shutdown.is_exiting() {
        state.aria2_config()
    } else {
        ensure_aria2_ready(&state)
            .await
            .map_err(classify_aria2_ready_error)?
    };
    let tasks = service
        .list_download_tasks(&config)
        .await
        .map_err(classify_task_error)?;
    Ok(Json(tasks))
}

async fn create_task(
    State(state): State<Arc<HttpAppState>>,
    ApiJson(payload): ApiJson<CreateDownloadTaskRequest>,
) -> Result<Json<DownloadTask>, ApiError> {
    let service = task_service(&state);
    service.ensure_not_exiting().map_err(classify_task_error)?;
    ensure_authorized_save_dir(&state, payload.save_dir.as_deref())?;
    let config = ensure_aria2_ready(&state)
        .await
        .map_err(classify_aria2_ready_error)?;
    let task = service
        .create_download_task(&config, payload)
        .await
        .map_err(classify_task_error)?;
    broadcast_tasks_snapshot(&state)
        .map_err(|error| ApiError::internal("tasks_snapshot_broadcast_failed", error))?;
    Ok(Json(task))
}

async fn create_batch_tasks(
    State(state): State<Arc<HttpAppState>>,
    ApiJson(payload): ApiJson<CreateBatchDownloadTasksRequest>,
) -> Result<(StatusCode, Json<CreateBatchDownloadTasksResponse>), ApiError> {
    let service = task_service(&state);
    service.ensure_not_exiting().map_err(classify_task_error)?;
    ensure_authorized_save_dir(&state, Some(&payload.save_dir))?;
    let config = ensure_aria2_ready(&state)
        .await
        .map_err(classify_aria2_ready_error)?;

    let urls = payload
        .urls
        .into_iter()
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty())
        .collect::<Vec<_>>();

    let mut created = Vec::new();
    let mut failed = Vec::new();
    if urls.is_empty() {
        failed.push(CreateBatchDownloadTaskFailure {
            input: String::new(),
            message: "请输入至少一个 HTTP / HTTPS 下载链接".to_string(),
        });
    }

    for url in urls {
        let request = CreateDownloadTaskRequest {
            url: url.clone(),
            file_name: None,
            save_dir: Some(payload.save_dir.clone()),
            source_type: DownloadTaskSourceType::Url,
            start_mode: payload.start_mode,
            category: payload.category.clone(),
            advanced_options: payload.advanced_options.clone(),
            aria2_options: serde_json::Map::new(),
        };
        match service.create_download_task(&config, request).await {
            Ok(task) => created.push(task),
            Err(error) => failed.push(CreateBatchDownloadTaskFailure {
                input: url,
                message: error,
            }),
        }
    }

    if !created.is_empty() {
        broadcast_tasks_snapshot(&state)
            .map_err(|error| ApiError::internal("tasks_snapshot_broadcast_failed", error))?;
    }

    let status = if created.is_empty() {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::OK
    };
    Ok((
        status,
        Json(CreateBatchDownloadTasksResponse { created, failed }),
    ))
}

async fn pause_task(
    State(state): State<Arc<HttpAppState>>,
    Path(task_id): Path<u64>,
) -> Result<Json<DownloadTask>, ApiError> {
    let service = task_service(&state);
    service.ensure_not_exiting().map_err(classify_task_error)?;
    let config = ensure_aria2_ready(&state)
        .await
        .map_err(classify_aria2_ready_error)?;
    let task = service
        .pause_download_task(&config, task_id)
        .await
        .map_err(classify_task_error)?;
    broadcast_tasks_snapshot(&state)
        .map_err(|error| ApiError::internal("tasks_snapshot_broadcast_failed", error))?;
    Ok(Json(task))
}

async fn resume_task(
    State(state): State<Arc<HttpAppState>>,
    Path(task_id): Path<u64>,
) -> Result<Json<DownloadTask>, ApiError> {
    let service = task_service(&state);
    service.ensure_not_exiting().map_err(classify_task_error)?;
    let config = ensure_aria2_ready(&state)
        .await
        .map_err(classify_aria2_ready_error)?;
    let task = service
        .resume_download_task(&config, task_id)
        .await
        .map_err(classify_task_error)?;
    broadcast_tasks_snapshot(&state)
        .map_err(|error| ApiError::internal("tasks_snapshot_broadcast_failed", error))?;
    Ok(Json(task))
}

async fn redownload_task(
    State(state): State<Arc<HttpAppState>>,
    Path(task_id): Path<u64>,
) -> Result<Json<DownloadTask>, ApiError> {
    let service = task_service(&state);
    service.ensure_not_exiting().map_err(classify_task_error)?;
    let config = ensure_aria2_ready(&state)
        .await
        .map_err(classify_aria2_ready_error)?;
    let task = service
        .redownload_download_task(&config, task_id)
        .await
        .map_err(classify_task_error)?;
    broadcast_tasks_snapshot(&state)
        .map_err(|error| ApiError::internal("tasks_snapshot_broadcast_failed", error))?;
    Ok(Json(task))
}

async fn delete_task(
    State(state): State<Arc<HttpAppState>>,
    Path(task_id): Path<u64>,
    Query(query): Query<DeleteTaskQuery>,
) -> Result<Json<DownloadTask>, ApiError> {
    let service = task_service(&state);
    service.ensure_not_exiting().map_err(classify_task_error)?;
    let config = ensure_aria2_ready(&state)
        .await
        .map_err(classify_aria2_ready_error)?;
    let task = service
        .delete_download_task(&config, task_id, query.delete_files.unwrap_or(false))
        .await
        .map_err(classify_task_error)?;
    broadcast_tasks_snapshot(&state)
        .map_err(|error| ApiError::internal("tasks_snapshot_broadcast_failed", error))?;
    Ok(Json(task))
}

async fn permanently_delete_task(
    State(state): State<Arc<HttpAppState>>,
    Path(task_id): Path<u64>,
) -> Result<StatusCode, ApiError> {
    let service = task_service(&state);
    service
        .permanently_delete_removed_task(task_id)
        .await
        .map_err(classify_task_error)?;
    Ok(StatusCode::NO_CONTENT)
}

fn task_service(state: &HttpAppState) -> TaskService<'_> {
    TaskService::new(
        Box::new(SqliteTaskRepository::new(&state.core.database.pool)),
        &state.core.download_tasks,
        &state.core.next_task_id,
        &state.core.debug_logs,
        RuntimeGuard::new(&state.core.shutdown),
    )
}

fn ensure_authorized_save_dir(
    state: &HttpAppState,
    save_dir: Option<&str>,
) -> Result<(), ApiError> {
    let save_dir = save_dir
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::bad_request("save_dir_required", "请选择已授权的保存目录"))?;
    let accessible_paths = super::storage::load_accessible_paths(state)?;

    if accessible_paths.is_empty() {
        return Err(ApiError::bad_request(
            "no_accessible_paths",
            "未检测到已授权目录，请先在飞牛应用设置中添加读写文件夹授权",
        ));
    }
    if !accessible_paths.iter().any(|path| path == save_dir) {
        return Err(ApiError::bad_request(
            "save_dir_not_authorized",
            "保存目录不在飞牛已授权目录列表中",
        ));
    }

    Ok(())
}

fn classify_aria2_ready_error(error: String) -> ApiError {
    if error.contains("应用正在退出") {
        return ApiError::conflict("runtime_exiting", error);
    }
    if error.contains("端口范围")
        || error.contains("已被其他进程占用")
        || error.contains("RPC 未就绪")
    {
        return ApiError::conflict("aria2_runtime_conflict", error);
    }
    ApiError::internal("aria2_runtime_failed", error)
}

fn classify_task_error(error: String) -> ApiError {
    if error.contains("应用正在退出") {
        return ApiError::conflict("runtime_exiting", error);
    }
    if error.contains("下载任务不存在")
        || error.contains("只有已完成任务可以重新下载")
        || error.contains("URL")
        || error.contains("文件名")
        || error.contains("保存目录")
        || error.contains("只有已删除任务可以永久删除")
        || error.contains("拒绝删除")
        || error.contains("当前仅支持删除单文件")
    {
        return ApiError::bad_request("task_operation_failed", error);
    }
    ApiError::internal("task_operation_failed", error)
}

#[cfg(test)]
mod tests;
