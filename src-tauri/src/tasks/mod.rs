use crate::config::aria2::Aria2Config;
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
    pub error_message: Option<String>,
    pub created_at: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDownloadTaskRequest {
    pub url: String,
    pub file_name: Option<String>,
    pub save_dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedDownloadTask {
    pub url: String,
    pub file_name: String,
    pub save_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AddUriResponse {
    result: Option<String>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    message: String,
}

pub fn prepare_task(request: CreateDownloadTaskRequest) -> Result<PreparedDownloadTask, String> {
    let url = normalize_required(&request.url, "下载链接不能为空")?;
    validate_http_url(&url)?;

    Ok(PreparedDownloadTask {
        file_name: normalize_optional(request.file_name).unwrap_or_else(|| infer_file_name(&url)),
        save_dir: normalize_optional(request.save_dir),
        url,
    })
}

pub async fn add_uri_to_aria2(
    config: &Aria2Config,
    task: &PreparedDownloadTask,
) -> Result<String, String> {
    let request_body = build_add_uri_request(config, task);
    let response = reqwest::Client::new()
        .post(config.rpc_url())
        .json(&request_body)
        .send()
        .await
        .map_err(|error| format!("创建 Aria2 下载任务失败，RPC 不可用：{}", error))?;

    let rpc_response = response
        .json::<AddUriResponse>()
        .await
        .map_err(|error| format!("创建 Aria2 下载任务失败，响应解析失败：{}", error))?;

    if let Some(error) = rpc_response.error {
        return Err(format!("创建 Aria2 下载任务失败：{}", error.message));
    }

    rpc_response
        .result
        .filter(|gid| !gid.trim().is_empty())
        .ok_or_else(|| "创建 Aria2 下载任务失败：响应缺少 GID".to_string())
}

pub fn store_created_task(
    tasks: &Mutex<Vec<DownloadTask>>,
    next_id: &AtomicU64,
    prepared: PreparedDownloadTask,
    gid: String,
) -> Result<DownloadTask, String> {
    let task = DownloadTask {
        id: next_id.fetch_add(1, Ordering::Relaxed),
        file_name: prepared.file_name,
        save_dir: prepared.save_dir,
        url: prepared.url,
        gid: Some(gid),
        status: DownloadTaskStatus::Pending,
        total_length: 0,
        completed_length: 0,
        download_speed: 0,
        error_message: None,
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

fn build_add_uri_request(config: &Aria2Config, task: &PreparedDownloadTask) -> serde_json::Value {
    let mut params = Vec::new();
    if !config.rpc_secret.is_empty() {
        params.push(serde_json::json!(format!("token:{}", config.rpc_secret)));
    }

    params.push(serde_json::json!([task.url.clone()]));

    let mut options = serde_json::Map::new();
    if let Some(dir) = &task.save_dir {
        options.insert("dir".to_string(), serde_json::json!(dir));
    }
    if !task.file_name.trim().is_empty() {
        options.insert("out".to_string(), serde_json::json!(task.file_name));
    }
    params.push(serde_json::Value::Object(options));

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": "motrix-fnos-add-uri",
        "method": "aria2.addUri",
        "params": params,
    })
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

    fn test_config() -> Aria2Config {
        Aria2Config {
            aria2_path: None,
            binary_source: crate::config::aria2::Aria2BinarySource::Sidecar,
            sidecar_name: "aria2-next".to_string(),
            target_triple: "test-target".to_string(),
            rpc_host: "127.0.0.1".to_string(),
            rpc_port: 6800,
            rpc_secret: String::new(),
        }
    }

    #[test]
    fn prepare_task_accepts_https_url() {
        let task = prepare_task(CreateDownloadTaskRequest {
            url: " https://example.com/file.zip?token=1 ".to_string(),
            file_name: None,
            save_dir: Some(" /downloads ".to_string()),
        })
        .expect("https task should be prepared");

        assert_eq!(task.url, "https://example.com/file.zip?token=1");
        assert_eq!(task.file_name, "file.zip");
        assert_eq!(task.save_dir.as_deref(), Some("/downloads"));
    }

    #[test]
    fn prepare_task_rejects_non_http_url() {
        let error = prepare_task(CreateDownloadTaskRequest {
            url: "magnet:?xt=urn:btih:test".to_string(),
            file_name: None,
            save_dir: None,
        })
        .expect_err("non-http url should fail");

        assert!(error.contains("HTTP / HTTPS"));
    }

    #[test]
    fn store_created_task_persists_gid() {
        let tasks = Mutex::new(Vec::new());
        let next_id = AtomicU64::new(1);
        let task = store_created_task(
            &tasks,
            &next_id,
            PreparedDownloadTask {
                url: "https://example.com/file.zip".to_string(),
                file_name: "file.zip".to_string(),
                save_dir: None,
            },
            "abc123".to_string(),
        )
        .expect("task should be stored");

        assert_eq!(task.id, 1);
        assert_eq!(task.gid.as_deref(), Some("abc123"));
        assert_eq!(
            list_tasks(&tasks).expect("tasks should be readable").len(),
            1
        );
    }

    #[test]
    fn add_uri_request_contains_url_and_options() {
        let request = build_add_uri_request(
            &test_config(),
            &PreparedDownloadTask {
                url: "https://example.com/file.zip".to_string(),
                file_name: "custom.zip".to_string(),
                save_dir: Some("/downloads".to_string()),
            },
        );

        assert_eq!(request["method"], "aria2.addUri");
        assert_eq!(request["params"][0][0], "https://example.com/file.zip");
        assert_eq!(request["params"][1]["dir"], "/downloads");
        assert_eq!(request["params"][1]["out"], "custom.zip");
    }
}
