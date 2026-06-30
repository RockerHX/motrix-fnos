use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DownloadTaskStatus {
    Pending,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTask {
    pub id: u64,
    pub url: String,
    pub file_name: String,
    pub save_dir: Option<String>,
    pub gid: Option<String>,
    pub status: DownloadTaskStatus,
    pub total_length: u64,
    pub completed_length: u64,
    pub download_speed: u64,
    pub created_at: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDownloadTaskRequest {
    pub url: String,
    pub file_name: Option<String>,
    pub save_dir: Option<String>,
}

pub fn create_task(
    tasks: &Mutex<Vec<DownloadTask>>,
    next_id: &AtomicU64,
    request: CreateDownloadTaskRequest,
) -> Result<DownloadTask, String> {
    let url = normalize_required(&request.url, "下载链接不能为空")?;
    validate_http_url(&url)?;

    let task = DownloadTask {
        id: next_id.fetch_add(1, Ordering::Relaxed),
        file_name: normalize_optional(request.file_name).unwrap_or_else(|| infer_file_name(&url)),
        save_dir: normalize_optional(request.save_dir),
        url,
        gid: None,
        status: DownloadTaskStatus::Pending,
        total_length: 0,
        completed_length: 0,
        download_speed: 0,
        created_at: current_timestamp_ms(),
    };

    let mut guard = tasks
        .lock()
        .map_err(|_| "无法写入下载任务列表".to_string())?;
    guard.push(task.clone());

    Ok(task)
}

pub fn list_tasks(tasks: &Mutex<Vec<DownloadTask>>) -> Result<Vec<DownloadTask>, String> {
    tasks
        .lock()
        .map(|guard| guard.clone())
        .map_err(|_| "无法读取下载任务列表".to_string())
}

fn normalize_required(value: &str, empty_message: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(empty_message.to_string());
    }

    Ok(trimmed.to_string())
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn validate_http_url(url: &str) -> Result<(), String> {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Ok(());
    }

    Err("阶段 1 当前仅支持 HTTP / HTTPS 下载链接".to_string())
}

fn infer_file_name(url: &str) -> String {
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .trim_end_matches('/');

    path.rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("未命名下载任务")
        .to_string()
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_store() -> (Mutex<Vec<DownloadTask>>, AtomicU64) {
        (Mutex::new(Vec::new()), AtomicU64::new(1))
    }

    #[test]
    fn create_task_accepts_https_url() {
        let (tasks, next_id) = empty_store();
        let task = create_task(
            &tasks,
            &next_id,
            CreateDownloadTaskRequest {
                url: " https://example.com/file.zip?token=1 ".to_string(),
                file_name: None,
                save_dir: Some(" /downloads ".to_string()),
            },
        )
        .expect("https task should be created");

        assert_eq!(task.id, 1);
        assert_eq!(task.url, "https://example.com/file.zip?token=1");
        assert_eq!(task.file_name, "file.zip");
        assert_eq!(task.save_dir.as_deref(), Some("/downloads"));
        assert_eq!(task.status, DownloadTaskStatus::Pending);
        assert_eq!(
            list_tasks(&tasks).expect("tasks should be readable").len(),
            1
        );
    }

    #[test]
    fn create_task_rejects_non_http_url() {
        let (tasks, next_id) = empty_store();
        let error = create_task(
            &tasks,
            &next_id,
            CreateDownloadTaskRequest {
                url: "magnet:?xt=urn:btih:test".to_string(),
                file_name: None,
                save_dir: None,
            },
        )
        .expect_err("non-http url should fail");

        assert!(error.contains("HTTP / HTTPS"));
        assert!(list_tasks(&tasks)
            .expect("tasks should be readable")
            .is_empty());
    }
}
