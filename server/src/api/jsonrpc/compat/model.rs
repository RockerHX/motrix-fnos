use crate::tasks::{DownloadTask, DownloadTaskSourceType, DownloadTaskStatus};
use serde_json::{Map, Value};
use std::path::Component;
use std::path::Path;

pub(super) const SUPPORTED_KEYS: &[&str] = &[
    "gid",
    "status",
    "totalLength",
    "completedLength",
    "uploadLength",
    "downloadSpeed",
    "uploadSpeed",
    "connections",
    "numSeeders",
    "seeder",
    "errorCode",
    "errorMessage",
    "dir",
    "files",
    "bittorrent",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Aria2CompatFile {
    pub(super) index: u32,
    pub(super) path: String,
    pub(super) length: u64,
    pub(super) completed_length: u64,
    pub(super) selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Aria2CompatTask {
    pub(super) gid: String,
    pub(super) status: String,
    pub(super) total_length: u64,
    pub(super) completed_length: u64,
    pub(super) download_speed: u64,
    pub(super) error_code: String,
    pub(super) error_message: Option<String>,
    pub(super) dir: Option<String>,
    pub(super) files: Vec<Aria2CompatFile>,
    pub(super) bittorrent_name: Option<String>,
}

impl Aria2CompatTask {
    pub(super) fn from_download_task(task: &DownloadTask) -> Option<Self> {
        let gid = task
            .gid
            .as_deref()
            .map(str::trim)
            .filter(|gid| !gid.is_empty())?
            .to_string();
        let status = status_for_task(task)?;
        let save_dir = task.save_dir.trim();
        let save_dir_path = Path::new(save_dir);
        let expose_paths = !save_dir.is_empty() && save_dir_path.is_absolute();
        let files = if task.files.is_empty() {
            task.file_path
                .as_deref()
                .filter(|path| expose_paths && is_path_under(Path::new(path.trim()), save_dir_path))
                .map(|path| {
                    vec![Aria2CompatFile {
                        index: 1,
                        path: path.to_string(),
                        length: task.total_length,
                        completed_length: task.completed_length,
                        selected: true,
                    }]
                })
                .unwrap_or_default()
        } else {
            task.files
                .iter()
                .filter(|file| {
                    expose_paths && is_path_under(Path::new(file.path.trim()), save_dir_path)
                })
                .map(|file| Aria2CompatFile {
                    index: file.index,
                    path: file.path.clone(),
                    length: file.length,
                    completed_length: file.completed_length,
                    selected: file.selected,
                })
                .collect()
        };

        Some(Self {
            gid,
            status: status.to_string(),
            total_length: task.total_length,
            completed_length: task.completed_length,
            download_speed: task.download_speed,
            error_code: normalize_error_code(task.error_code.as_deref()),
            error_message: (task.status == DownloadTaskStatus::Error)
                .then_some(task.error_message.as_deref())
                .flatten()
                .map(str::trim)
                .filter(|message| !message.is_empty())
                .map(crate::debug_logs::redact_log_message),
            dir: expose_paths.then(|| save_dir.to_string()),
            files,
            bittorrent_name: match task.source_type {
                DownloadTaskSourceType::Torrent | DownloadTaskSourceType::Magnet => {
                    let name = task.file_name.trim();
                    (!name.is_empty()).then(|| name.to_string())
                }
                DownloadTaskSourceType::Url => None,
            },
        })
    }

    pub(super) fn to_value(&self, keys: &[String]) -> Value {
        let mut value = Map::new();
        for key in keys {
            match key.as_str() {
                "gid" => insert_string(&mut value, key, &self.gid),
                "status" => insert_string(&mut value, key, &self.status),
                "totalLength" => insert_u64(&mut value, key, self.total_length),
                "completedLength" => insert_u64(&mut value, key, self.completed_length),
                "uploadLength" => insert_string(&mut value, key, "0"),
                "downloadSpeed" => insert_u64(&mut value, key, self.download_speed),
                "uploadSpeed" => insert_string(&mut value, key, "0"),
                "connections" => insert_string(&mut value, key, "0"),
                "numSeeders" => insert_string(&mut value, key, "0"),
                "seeder" => {
                    value.insert(key.clone(), Value::String("false".to_string()));
                }
                "errorCode" => insert_string(&mut value, key, &self.error_code),
                "errorMessage" => {
                    if let Some(message) = self.error_message.as_deref() {
                        insert_string(&mut value, key, message);
                    }
                }
                "dir" => {
                    if let Some(dir) = self.dir.as_deref() {
                        insert_string(&mut value, key, dir);
                    }
                }
                "files" => {
                    value.insert(key.clone(), files_value(&self.files));
                }
                "bittorrent" => {
                    if let Some(name) = self.bittorrent_name.as_deref() {
                        value.insert(key.clone(), serde_json::json!({"info": {"name": name}}));
                    }
                }
                _ => {}
            }
        }
        Value::Object(value)
    }
}

pub(super) fn serialize_tasks(tasks: &[DownloadTask], keys: &[String]) -> Value {
    Value::Array(
        tasks
            .iter()
            .filter_map(Aria2CompatTask::from_download_task)
            .map(|task| task.to_value(keys))
            .collect(),
    )
}

fn normalize_error_code(error_code: Option<&str>) -> String {
    error_code
        .map(str::trim)
        .filter(|code| !code.is_empty() && code.bytes().all(|byte| byte.is_ascii_digit()))
        .unwrap_or("0")
        .to_string()
}

fn is_path_under(path: &Path, save_dir: &Path) -> bool {
    path.is_absolute()
        && path.starts_with(save_dir)
        && !path
            .components()
            .any(|component| component == Component::ParentDir)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Aria2GlobalStat {
    pub(super) download_speed: u64,
    pub(super) num_active: u64,
    pub(super) num_waiting: u64,
    pub(super) num_stopped: u64,
}

impl Aria2GlobalStat {
    pub(super) fn empty() -> Self {
        Self {
            download_speed: 0,
            num_active: 0,
            num_waiting: 0,
            num_stopped: 0,
        }
    }

    pub(super) fn to_value(&self) -> Value {
        serde_json::json!({
            "downloadSpeed": self.download_speed.to_string(),
            "uploadSpeed": "0",
            "numActive": self.num_active.to_string(),
            "numWaiting": self.num_waiting.to_string(),
            "numStopped": self.num_stopped.to_string(),
            "numStoppedTotal": self.num_stopped.to_string(),
        })
    }
}

fn status_for_task(task: &DownloadTask) -> Option<&'static str> {
    match task.status {
        DownloadTaskStatus::Pending => Some("waiting"),
        DownloadTaskStatus::Active => Some("active"),
        DownloadTaskStatus::Paused => Some("paused"),
        DownloadTaskStatus::Complete => Some("complete"),
        DownloadTaskStatus::Error => Some("error"),
        DownloadTaskStatus::Removed => None,
    }
}

fn insert_string(value: &mut Map<String, Value>, key: &str, item: &str) {
    value.insert(key.to_string(), Value::String(item.to_string()));
}

fn insert_u64(value: &mut Map<String, Value>, key: &str, item: u64) {
    insert_string(value, key, &item.to_string());
}

fn files_value(files: &[Aria2CompatFile]) -> Value {
    Value::Array(
        files
            .iter()
            .map(|file| {
                serde_json::json!({
                    "index": file.index.to_string(),
                    "path": file.path,
                    "length": file.length.to_string(),
                    "completedLength": file.completed_length.to_string(),
                    "selected": if file.selected { "true" } else { "false" },
                })
            })
            .collect(),
    )
}
