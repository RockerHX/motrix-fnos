use super::bad_request_with_log;
use crate::api::error::ApiError;
use crate::app::HttpAppState;
use crate::tasks::{CreateTaskAdvancedOptions, DownloadTaskStartMode, PublicDownloadTask};
use axum::extract::Multipart;
use serde::{Deserialize, Serialize};

const MAX_TORRENT_FILE_SIZE: usize = 10 * 1024 * 1024;

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(super) struct DeleteTaskQuery {
    pub(super) delete_files: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
pub(super) struct ListTasksQuery {
    pub(super) status: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CreateBatchDownloadTasksRequest {
    pub(super) urls: Vec<String>,
    pub(super) save_dir: String,
    #[serde(default)]
    pub(super) start_mode: DownloadTaskStartMode,
    pub(super) category: Option<String>,
    #[serde(default)]
    pub(super) advanced_options: CreateTaskAdvancedOptions,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CreateBatchDownloadTasksResponse {
    pub(super) created: Vec<PublicDownloadTask>,
    pub(super) failed: Vec<CreateBatchDownloadTaskFailure>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CreateBatchDownloadTaskFailure {
    pub(super) input: String,
    pub(super) message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CreateTorrentUploadRequest {
    pub(super) save_dir: String,
    #[serde(default)]
    pub(super) start_mode: DownloadTaskStartMode,
    pub(super) category: Option<String>,
    #[serde(default)]
    pub(super) advanced_options: CreateTaskAdvancedOptions,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ConfirmTaskFilesRequest {
    pub(super) selected_file_indexes: Vec<u32>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateTaskProxyRequest {
    pub(super) enabled: bool,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(super) struct TaskProxyOverrideRequest {
    #[serde(default)]
    pub(super) use_proxy: Option<bool>,
}

pub(super) fn parse_task_proxy_override_body(
    body: &[u8],
) -> Result<Option<TaskProxyOverrideRequest>, ApiError> {
    if body.is_empty() {
        return Ok(None);
    }
    serde_json::from_slice(body).map(Some).map_err(|error| {
        ApiError::bad_request("invalid_json", format!("请求体 JSON 无效：{}", error))
    })
}

pub(super) enum ListTasksFilter {
    Visible,
    Removed,
}

impl ListTasksQuery {
    pub(super) fn filter(&self) -> Result<ListTasksFilter, ApiError> {
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

pub(super) struct ParsedTorrentUpload {
    pub(super) file_name: String,
    pub(super) data: Vec<u8>,
    pub(super) request: CreateTorrentUploadRequest,
}

pub(super) async fn parse_torrent_multipart(
    mut multipart: Multipart,
    state: &HttpAppState,
) -> Result<ParsedTorrentUpload, ApiError> {
    let mut file_name = None;
    let mut data = None;
    let mut request = None;

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        bad_request_with_log(
            state,
            "invalid_multipart",
            format!("上传表单无效：{}", error),
        )
    })? {
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "torrent" => {
                let next_file_name = field
                    .file_name()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or("download.torrent")
                    .to_string();
                let bytes = field.bytes().await.map_err(|error| {
                    bad_request_with_log(
                        state,
                        "invalid_torrent_file",
                        format!("读取种子文件失败：{}", error),
                    )
                })?;
                if bytes.is_empty() {
                    return Err(bad_request_with_log(
                        state,
                        "torrent_empty",
                        "种子文件不能为空",
                    ));
                }
                if bytes.len() > MAX_TORRENT_FILE_SIZE {
                    return Err(bad_request_with_log(
                        state,
                        "torrent_too_large",
                        "种子文件不能超过 10 MiB",
                    ));
                }
                file_name = Some(next_file_name);
                data = Some(bytes.to_vec());
            }
            "request" => {
                let text = field.text().await.map_err(|error| {
                    bad_request_with_log(
                        state,
                        "invalid_torrent_request",
                        format!("读取种子请求失败：{}", error),
                    )
                })?;
                request = Some(
                    serde_json::from_str::<CreateTorrentUploadRequest>(&text).map_err(|error| {
                        bad_request_with_log(
                            state,
                            "invalid_torrent_request",
                            format!("种子请求 JSON 无效：{}", error),
                        )
                    })?,
                );
            }
            _ => {}
        }
    }

    Ok(ParsedTorrentUpload {
        file_name: file_name
            .ok_or_else(|| bad_request_with_log(state, "torrent_required", "请选择种子文件"))?,
        data: data
            .ok_or_else(|| bad_request_with_log(state, "torrent_required", "请选择种子文件"))?,
        request: request.ok_or_else(|| {
            bad_request_with_log(state, "torrent_request_required", "缺少种子任务请求参数")
        })?,
    })
}
