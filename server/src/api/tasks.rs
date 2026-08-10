use crate::api::error::ApiError;
use crate::api::extract::ApiJson;
use crate::app::HttpAppState;
use crate::runtime::{broadcast_tasks_snapshot, spawn_file_cleanup_worker};
use crate::storage::TaskSaveDirError;
use crate::tasks::repository::SqliteTaskRepository;
use crate::tasks::service::{RuntimeGuard, TaskService};
use crate::tasks::{
    CreateDownloadTaskRequest, CreateTorrentDownloadTaskRequest, DownloadTaskSourceType,
    PublicDownloadTask,
};
use axum::body::Bytes;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use std::sync::Arc;

#[path = "tasks/context.rs"]
mod context;
#[path = "tasks/request.rs"]
mod request;

use context::TaskMutationContext;
use request::*;

pub fn routes() -> Router<Arc<HttpAppState>> {
    Router::new()
        .route("/tasks", get(list_tasks).post(create_task))
        .route("/tasks/batch", post(create_batch_tasks))
        .route("/tasks/:id/confirm", post(confirm_task_files))
        .route("/tasks/:id/proxy", put(update_task_proxy))
        .route("/tasks/:id/pause", post(pause_task))
        .route("/tasks/:id/resume", post(resume_task))
        .route("/tasks/:id/redownload", post(redownload_task))
        .route("/tasks/:id/restore", post(restore_task))
        .route("/tasks/:id/permanent", delete(permanently_delete_task))
        .route("/tasks/:id", delete(delete_task))
}

pub fn torrent_routes() -> Router<Arc<HttpAppState>> {
    Router::new().route("/tasks/torrent", post(create_torrent_task))
}

async fn list_tasks(
    State(state): State<Arc<HttpAppState>>,
    Query(query): Query<ListTasksQuery>,
) -> Result<Json<Vec<PublicDownloadTask>>, ApiError> {
    let service = task_service(&state);
    if matches!(query.filter()?, ListTasksFilter::Removed) {
        let tasks = service
            .list_removed_download_tasks()
            .map_err(classify_task_error)?;
        return Ok(Json(tasks.into_iter().map(Into::into).collect()));
    }

    // 退出期间只读取最后已知配置，不能为了列表查询重新启动已经进入清理流程的 Aria2。
    let tasks = service
        .list_download_task_snapshot()
        .map_err(classify_task_error)?;
    Ok(Json(tasks.into_iter().map(Into::into).collect()))
}

async fn create_task(
    State(state): State<Arc<HttpAppState>>,
    ApiJson(payload): ApiJson<CreateDownloadTaskRequest>,
) -> Result<Json<PublicDownloadTask>, ApiError> {
    validate_create_preconditions(&state, payload.save_dir.as_deref())?;
    task_service(&state)
        .validate_create_task_proxy(&payload.advanced_options, &payload.aria2_options)
        .await
        .map_err(classify_task_error)?;
    let context =
        TaskMutationContext::prepare_for_create(&state, payload.save_dir.as_deref()).await?;
    let task = context
        .service
        .create_download_task(&context.config, payload)
        .await
        .map_err(classify_task_error)?;
    context.finish(task)
}

async fn create_batch_tasks(
    State(state): State<Arc<HttpAppState>>,
    ApiJson(payload): ApiJson<CreateBatchDownloadTasksRequest>,
) -> Result<(StatusCode, Json<CreateBatchDownloadTasksResponse>), ApiError> {
    let urls = payload
        .urls
        .into_iter()
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty())
        .collect::<Vec<_>>();

    let mut created = Vec::new();
    let mut failed = Vec::new();
    validate_create_preconditions(&state, Some(&payload.save_dir))?;
    if urls.is_empty() {
        state.core.debug_logs.warn(
            "api.tasks",
            "批量创建任务失败：请输入至少一个 HTTP / HTTPS 下载链接",
        );
        failed.push(CreateBatchDownloadTaskFailure {
            input: String::new(),
            message: "请输入至少一个 HTTP / HTTPS 下载链接".to_string(),
        });
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(CreateBatchDownloadTasksResponse { created, failed }),
        ));
    }
    if let Err(error) = task_service(&state)
        .validate_create_task_proxy(&payload.advanced_options, &serde_json::Map::new())
        .await
    {
        if !is_proxy_request_error(&error) {
            return Err(classify_task_error(error));
        }
        failed.extend(
            urls.into_iter()
                .map(|input| CreateBatchDownloadTaskFailure {
                    input,
                    message: error.clone(),
                }),
        );
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(CreateBatchDownloadTasksResponse { created, failed }),
        ));
    }
    let context = TaskMutationContext::prepare_for_create(&state, Some(&payload.save_dir)).await?;

    // 每个 URL 都是独立任务：单条失败只进入 failed，已经创建并持久化的任务不回滚。
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
        match context
            .service
            .create_download_task(&context.config, request)
            .await
        {
            Ok(task) => created.push(task.into()),
            Err(error) => failed.push(CreateBatchDownloadTaskFailure {
                input: url,
                message: error,
            }),
        }
    }

    if !created.is_empty() {
        context.broadcast_snapshot()?;
    }

    // 至少创建一条即返回成功并由响应体携带失败项；全部失败才把整个请求标记为参数错误。
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

async fn create_torrent_task(
    State(state): State<Arc<HttpAppState>>,
    multipart: Multipart,
) -> Result<Json<PublicDownloadTask>, ApiError> {
    let upload = parse_torrent_multipart(multipart, &state).await?;
    validate_create_preconditions(&state, Some(&upload.request.save_dir))?;
    task_service(&state)
        .validate_create_task_proxy(&upload.request.advanced_options, &serde_json::Map::new())
        .await
        .map_err(classify_task_error)?;
    let context =
        TaskMutationContext::prepare_for_create(&state, Some(&upload.request.save_dir)).await?;
    let task = context
        .service
        .create_torrent_download_task(
            &context.config,
            CreateTorrentDownloadTaskRequest {
                torrent_file_name: upload.file_name,
                torrent_data: upload.data,
                save_dir: upload.request.save_dir,
                start_mode: upload.request.start_mode,
                category: upload.request.category,
                advanced_options: upload.request.advanced_options,
            },
        )
        .await
        .map_err(classify_task_error)?;
    context.finish(task)
}

async fn pause_task(
    State(state): State<Arc<HttpAppState>>,
    Path(task_id): Path<u64>,
) -> Result<Json<PublicDownloadTask>, ApiError> {
    let context = TaskMutationContext::prepare(&state).await?;
    let task = context
        .service
        .pause_download_task(&context.config, task_id)
        .await
        .map_err(classify_task_error)?;
    context.finish(task)
}

async fn update_task_proxy(
    State(state): State<Arc<HttpAppState>>,
    Path(task_id): Path<u64>,
    ApiJson(payload): ApiJson<UpdateTaskProxyRequest>,
) -> Result<Json<PublicDownloadTask>, ApiError> {
    let service = task_service(&state);
    let config = state.aria2_runtime_snapshot().map(|_| state.aria2_config());
    let task = service
        .update_download_task_proxy(config.as_ref(), task_id, payload.enabled)
        .await
        .map_err(classify_task_proxy_error)?;
    broadcast_tasks_snapshot(&state)
        .map_err(|error| ApiError::internal("tasks_snapshot_broadcast_failed", error))?;
    Ok(Json(task.into()))
}

async fn confirm_task_files(
    State(state): State<Arc<HttpAppState>>,
    Path(task_id): Path<u64>,
    ApiJson(payload): ApiJson<ConfirmTaskFilesRequest>,
) -> Result<Json<PublicDownloadTask>, ApiError> {
    let context = TaskMutationContext::prepare(&state).await?;
    let task = context
        .service
        .confirm_download_task_files(&context.config, task_id, payload.selected_file_indexes)
        .await
        .map_err(classify_task_error)?;
    context.finish(task)
}

async fn resume_task(
    State(state): State<Arc<HttpAppState>>,
    Path(task_id): Path<u64>,
) -> Result<Json<PublicDownloadTask>, ApiError> {
    let context = TaskMutationContext::prepare(&state).await?;
    let task = context
        .service
        .resume_download_task(&context.config, task_id)
        .await
        .map_err(classify_task_error)?;
    context.finish(task)
}

async fn redownload_task(
    State(state): State<Arc<HttpAppState>>,
    Path(task_id): Path<u64>,
    body: Bytes,
) -> Result<Json<PublicDownloadTask>, ApiError> {
    let use_proxy = parse_task_proxy_override_body(&body)?.and_then(|payload| payload.use_proxy);
    let context = TaskMutationContext::prepare(&state).await?;
    let task = context
        .service
        .redownload_download_task(&context.config, task_id, use_proxy)
        .await
        .map_err(classify_task_error)?;
    context.finish(task)
}

async fn restore_task(
    State(state): State<Arc<HttpAppState>>,
    Path(task_id): Path<u64>,
    body: Bytes,
) -> Result<Json<PublicDownloadTask>, ApiError> {
    let use_proxy = parse_task_proxy_override_body(&body)?.and_then(|payload| payload.use_proxy);
    let context = TaskMutationContext::prepare(&state).await?;
    let task = context
        .service
        .restore_removed_task(&context.config, task_id, use_proxy)
        .await
        .map_err(classify_task_error)?;
    context.finish(task)
}

async fn delete_task(
    State(state): State<Arc<HttpAppState>>,
    Path(task_id): Path<u64>,
    Query(query): Query<DeleteTaskQuery>,
) -> Result<Json<PublicDownloadTask>, ApiError> {
    let context = TaskMutationContext::prepare(&state).await?;
    let task = context
        .service
        .delete_download_task(
            &context.config,
            task_id,
            query.delete_files.unwrap_or(false),
        )
        .await
        .map_err(classify_task_error)?;
    let response = context.finish(task)?;
    if query.delete_files.unwrap_or(false) {
        spawn_file_cleanup_worker(Arc::clone(&state)).await;
    }
    Ok(response)
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
        &state.core.app_data_dir,
        &state.core.debug_logs,
        &state.aria2_rpc,
        &state.aria2_lifecycle,
        &state.download_proxy_update_lock,
        RuntimeGuard::new(&state.core.shutdown),
    )
}

fn validate_create_preconditions(
    state: &HttpAppState,
    save_dir: Option<&str>,
) -> Result<(), ApiError> {
    task_service(state)
        .ensure_not_exiting()
        .map_err(classify_task_error)?;
    ensure_authorized_save_dir(state, save_dir)
}

fn ensure_authorized_save_dir(
    state: &HttpAppState,
    save_dir: Option<&str>,
) -> Result<(), ApiError> {
    let accessible_paths = super::storage::load_accessible_paths(state)?;
    crate::storage::validate_task_save_dir(save_dir, &accessible_paths).map_err(|error| {
        let (code, message) = match error {
            TaskSaveDirError::Required => ("save_dir_required", "请选择已授权的保存目录"),
            TaskSaveDirError::NoAccessiblePaths => (
                "no_accessible_paths",
                "未检测到已授权目录，请先在飞牛应用设置中添加读写文件夹授权",
            ),
            TaskSaveDirError::Unauthorized => (
                "save_dir_not_authorized",
                "保存目录不在飞牛已授权目录列表中",
            ),
        };
        let log_message = match error {
            TaskSaveDirError::Unauthorized => format!(
                "保存目录校验失败：未授权目录 {}",
                save_dir.unwrap_or_default()
            ),
            _ => format!("保存目录校验失败：{}", message),
        };
        state.core.debug_logs.warn("storage.auth", log_message);
        ApiError::bad_request(code, message)
    })
}

fn bad_request_with_log(
    state: &HttpAppState,
    code: impl Into<String>,
    message: impl Into<String>,
) -> ApiError {
    let message = message.into();
    state
        .core
        .debug_logs
        .warn("api.tasks", format!("任务接口请求校验失败：{}", message));
    ApiError::bad_request(code, message)
}

fn classify_aria2_ready_error(error: String) -> ApiError {
    if error.contains("应用正在退出") {
        return ApiError::conflict("runtime_exiting", error);
    }
    if error.contains("生命周期转换超时") || error.contains("生命周期请求被拒绝") {
        return ApiError::conflict("aria2_busy", error);
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
    if error.contains("生命周期转换超时") || error.contains("生命周期请求被拒绝") {
        return ApiError::conflict("aria2_busy", error);
    }
    if error.contains("已有操作正在进行") {
        return ApiError::conflict("task_operation_conflict", error);
    }
    if error.contains("file_cleanup_pending") {
        return ApiError::conflict("file_cleanup_pending", error);
    }
    if error.contains("代理选择冲突") {
        return ApiError::bad_request("proxy_conflict", error);
    }
    if error.contains("未配置下载代理") {
        return ApiError::bad_request("proxy_not_configured", error);
    }
    if error.contains("代理地址") || error.contains("代理协议") || error.contains("代理端口")
    {
        return ApiError::bad_request("proxy_invalid", error);
    }
    // 当前 service 使用中文错误文本区分可修正请求；新增或调整领域错误时必须同步检查这里的 HTTP 分类。
    if error.contains("下载任务不存在")
        || error.contains("只有已完成任务可以重新下载")
        || error.contains("请先确认")
        || error.contains("请至少选择")
        || error.contains("不需要确认")
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

fn classify_task_proxy_error(error: String) -> ApiError {
    if error.contains("应用正在退出") {
        return ApiError::conflict("runtime_exiting", error);
    }
    if error.contains("下载任务不存在") {
        return ApiError::not_found("task_not_found", error);
    }
    if error.contains("已有操作正在进行") {
        return ApiError::conflict("task_operation_conflict", error);
    }
    if error.contains("正在切换运行状态")
        || error.contains("生命周期请求被拒绝")
        || error.contains("生命周期转换超时")
    {
        return ApiError::conflict("runtime_transition", error);
    }
    if error.contains("未配置下载代理") {
        return ApiError::bad_request("proxy_not_configured", error);
    }
    if error.contains("更新任务选项失败") {
        return ApiError::bad_gateway("proxy_apply_failed", error);
    }
    ApiError::internal("task_proxy_update_failed", error)
}

fn is_proxy_request_error(error: &str) -> bool {
    error.contains("代理选择冲突")
        || error.contains("未配置下载代理")
        || error.contains("代理地址")
        || error.contains("代理协议")
        || error.contains("代理端口")
}

#[cfg(test)]
mod tests;
