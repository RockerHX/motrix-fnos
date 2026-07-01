use crate::config::aria2::{Aria2BinarySource, Aria2Config};
use crate::database::AppDatabase;
use crate::debug_logs::DebugLogStore;
use crate::tasks::DownloadTask;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Mutex;
use tauri_plugin_shell::process::CommandChild;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Aria2RuntimeInfo {
    pub pid: u32,
    pub actual_port: u16,
    pub rpc_secret: String,
    pub rpc_endpoint: String,
    pub binary_source: Aria2BinarySource,
    #[serde(default)]
    pub sidecar_name: Option<String>,
    #[serde(default)]
    pub app_data_dir: Option<String>,
    #[serde(default)]
    pub aria2_session_path: Option<String>,
    #[serde(default)]
    pub aria2_log_path: Option<String>,
    #[serde(default)]
    pub launch_args: Option<Vec<String>>,
}

pub const ARIA2_RUNTIME_FILE_NAME: &str = "aria2-runtime.json";
pub const ARIA2_RUNTIME_DIR_NAME: &str = "aria2";
pub const ARIA2_SESSION_FILE_NAME: &str = "aria2.session";
pub const ARIA2_LOG_FILE_NAME: &str = "aria2.log";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aria2RuntimePaths {
    pub runtime_dir: PathBuf,
    pub session_path: PathBuf,
    pub log_path: PathBuf,
}

pub fn app_data_dir_from_database_path(database_path: &Path) -> PathBuf {
    database_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn aria2_runtime_path(database_path: &Path) -> PathBuf {
    database_path
        .parent()
        .map(|parent| parent.join(ARIA2_RUNTIME_FILE_NAME))
        .unwrap_or_else(|| PathBuf::from(ARIA2_RUNTIME_FILE_NAME))
}

pub fn aria2_runtime_paths(app_data_dir: &Path) -> Aria2RuntimePaths {
    let runtime_dir = app_data_dir.join(ARIA2_RUNTIME_DIR_NAME);
    Aria2RuntimePaths {
        session_path: runtime_dir.join(ARIA2_SESSION_FILE_NAME),
        log_path: runtime_dir.join(ARIA2_LOG_FILE_NAME),
        runtime_dir,
    }
}

pub fn write_aria2_runtime_record(path: &Path, runtime: &Aria2RuntimeInfo) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "创建 Aria2 运行态目录失败：{}（{}）",
                parent.display(),
                error
            )
        })?;
    }
    let content = serde_json::to_string_pretty(runtime)
        .map_err(|error| format!("序列化 Aria2 运行态失败：{}", error))?;
    fs::write(path, content)
        .map_err(|error| format!("写入 Aria2 运行态文件失败：{}（{}）", path.display(), error))
}

pub fn read_aria2_runtime_record(path: &Path) -> Result<Option<Aria2RuntimeInfo>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)
        .map_err(|error| format!("读取 Aria2 运行态文件失败：{}（{}）", path.display(), error))?;
    serde_json::from_str(&content)
        .map(Some)
        .map_err(|error| format!("解析 Aria2 运行态文件失败：{}", error))
}

fn ensure_aria2_session_file(path: &Path) -> Result<(), String> {
    if path.exists() && !path.is_file() {
        return Err(format!(
            "Aria2 session 路径已存在但不是文件：{}",
            path.display()
        ));
    }

    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map(|_| ())
        .map_err(|error| {
            format!(
                "创建 Aria2 session 文件失败：{}（{}）",
                path.display(),
                error
            )
        })
}

pub fn remove_aria2_runtime_record(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "删除 Aria2 运行态文件失败：{}（{}）",
            path.display(),
            error
        )),
    }
}

pub enum ManagedAria2Process {
    External(Child),
    Sidecar(CommandChild),
}

impl ManagedAria2Process {
    pub fn id(&self) -> u32 {
        match self {
            Self::External(child) => child.id(),
            Self::Sidecar(child) => child.pid(),
        }
    }

    pub fn kill(self) -> Result<(), String> {
        match self {
            Self::External(mut child) => {
                child.kill().map_err(|error| error.to_string())?;
                let _ = child.wait();
                Ok(())
            }
            Self::Sidecar(child) => child.kill().map_err(|error| error.to_string()),
        }
    }
}

pub struct AppState {
    pub aria2_process: Mutex<Option<ManagedAria2Process>>,
    pub aria2_runtime: Mutex<Option<Aria2RuntimeInfo>>,
    pub download_tasks: Mutex<Vec<DownloadTask>>,
    pub database: AppDatabase,
    pub app_data_dir: PathBuf,
    pub aria2_runtime_path: PathBuf,
    pub debug_logs: DebugLogStore,
    pub next_task_id: AtomicU64,
    pub notified_task_events: Mutex<HashSet<String>>,
    pub is_exiting: AtomicBool,
}

impl AppState {
    pub fn new(
        database: AppDatabase,
        download_tasks: Vec<DownloadTask>,
        next_task_id: u64,
    ) -> Self {
        let restored_count = download_tasks.len();
        let app_data_dir = app_data_dir_from_database_path(&database.path);
        let aria2_runtime_path = aria2_runtime_path(&database.path);
        let state = Self {
            aria2_process: Mutex::new(None),
            aria2_runtime: Mutex::new(None),
            download_tasks: Mutex::new(download_tasks),
            app_data_dir,
            aria2_runtime_path,
            database,
            debug_logs: DebugLogStore::default(),
            next_task_id: AtomicU64::new(next_task_id),
            notified_task_events: Mutex::new(HashSet::new()),
            is_exiting: AtomicBool::new(false),
        };
        state
            .debug_logs
            .info("app", "应用启动，调试日志队列已初始化");
        state.debug_logs.info(
            "database",
            format!("SQLite 数据库已初始化：{}", state.database.path.display()),
        );
        state.debug_logs.info(
            "tasks.restore",
            format!(
                "已从 SQLite 恢复 {} 个任务，下一个任务 ID {}",
                restored_count, next_task_id
            ),
        );
        state
    }

    pub fn aria2_runtime_snapshot(&self) -> Option<Aria2RuntimeInfo> {
        self.aria2_runtime
            .lock()
            .ok()
            .and_then(|runtime| runtime.clone())
    }

    pub fn set_aria2_runtime(&self, runtime: Aria2RuntimeInfo) -> Result<(), String> {
        write_aria2_runtime_record(&self.aria2_runtime_path, &runtime)?;
        let mut guard = self
            .aria2_runtime
            .lock()
            .map_err(|_| "无法写入 Aria2 运行态".to_string())?;
        *guard = Some(runtime);
        Ok(())
    }

    pub fn clear_aria2_runtime(&self) {
        if let Ok(mut runtime) = self.aria2_runtime.lock() {
            *runtime = None;
        }
        let _ = remove_aria2_runtime_record(&self.aria2_runtime_path);
    }

    pub fn load_saved_aria2_runtime(&self) -> Option<Aria2RuntimeInfo> {
        read_aria2_runtime_record(&self.aria2_runtime_path)
            .ok()
            .flatten()
    }

    pub fn aria2_config(&self) -> Aria2Config {
        let mut config = Aria2Config::from_env();
        if let Some(runtime) = self.aria2_runtime_snapshot() {
            config.rpc_port = runtime.actual_port;
            config.rpc_secret = runtime.rpc_secret;
            config.session_path = runtime.aria2_session_path.clone();
            config.log_path = runtime.aria2_log_path.clone();
        }
        config
    }

    pub fn with_aria2_runtime_paths(&self, mut config: Aria2Config) -> Result<Aria2Config, String> {
        let paths = aria2_runtime_paths(&self.app_data_dir);
        fs::create_dir_all(&paths.runtime_dir).map_err(|error| {
            format!(
                "创建 Aria2 runtime 目录失败：{}（{}）",
                paths.runtime_dir.display(),
                error
            )
        })?;
        ensure_aria2_session_file(&paths.session_path)?;
        self.debug_logs.info(
            "aria2.runtime",
            format!("Aria2 runtime 目录已准备：{}", paths.runtime_dir.display()),
        );
        config.session_path = Some(paths.session_path.display().to_string());
        config.log_path = Some(paths.log_path.display().to_string());
        Ok(config)
    }

    pub fn build_aria2_runtime_info(
        &self,
        pid: u32,
        config: &Aria2Config,
        source: Aria2BinarySource,
        launch_args: Vec<String>,
    ) -> Aria2RuntimeInfo {
        Aria2RuntimeInfo {
            pid,
            actual_port: config.rpc_port,
            rpc_secret: config.rpc_secret.clone(),
            rpc_endpoint: config.rpc_url(),
            binary_source: source,
            sidecar_name: Some(config.sidecar_name.clone()),
            app_data_dir: Some(self.app_data_dir.display().to_string()),
            aria2_session_path: config.session_path.clone(),
            aria2_log_path: config.log_path.clone(),
            launch_args: Some(launch_args),
        }
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        if let Ok(runtime) = self.aria2_runtime.get_mut() {
            *runtime = None;
        }
        if let Ok(process) = self.aria2_process.get_mut() {
            if let Some(child) = process.take() {
                let pid = child.id();
                let _ = child.kill();
                let _ = crate::aria2::terminate_process(pid);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aria2_runtime_paths_use_app_data_aria2_directory() {
        let app_data_dir = PathBuf::from("/tmp/motrix-fnos-app-data");
        let paths = aria2_runtime_paths(&app_data_dir);

        assert_eq!(paths.runtime_dir, app_data_dir.join("aria2"));
        assert_eq!(paths.session_path, app_data_dir.join("aria2/aria2.session"));
        assert_eq!(paths.log_path, app_data_dir.join("aria2/aria2.log"));
    }

    #[test]
    fn ensure_aria2_session_file_creates_missing_file_without_truncating() {
        let path = std::env::temp_dir().join(format!(
            "motrix-fnos-session-{}.session",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be valid")
                .as_millis()
        ));

        ensure_aria2_session_file(&path).expect("session file should be created");
        assert!(path.is_file());
        fs::write(&path, b"content").expect("session content should write");
        ensure_aria2_session_file(&path).expect("existing session file should be kept");
        assert_eq!(
            fs::read_to_string(&path).expect("session should read"),
            "content"
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn old_runtime_record_without_identity_fields_still_reads() {
        let path = std::env::temp_dir().join(format!(
            "motrix-fnos-old-runtime-{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be valid")
                .as_millis()
        ));
        std::fs::write(
            &path,
            r#"{
  "pid": 42,
  "actualPort": 6800,
  "rpcSecret": "secret",
  "rpcEndpoint": "http://127.0.0.1:6800/jsonrpc",
  "binarySource": "sidecar"
}
"#,
        )
        .expect("old runtime fixture should write");

        let restored = read_aria2_runtime_record(&path)
            .expect("old runtime should read")
            .expect("old runtime should exist");

        assert_eq!(restored.pid, 42);
        assert_eq!(restored.actual_port, 6800);
        assert_eq!(restored.binary_source, Aria2BinarySource::Sidecar);
        assert!(restored.sidecar_name.is_none());
        assert!(restored.app_data_dir.is_none());
        assert!(restored.launch_args.is_none());

        remove_aria2_runtime_record(&path).expect("old runtime should remove");
    }

    #[test]
    fn runtime_record_round_trips_and_removes() {
        let path = std::env::temp_dir().join(format!(
            "motrix-fnos-runtime-{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be valid")
                .as_millis()
        ));
        let runtime = Aria2RuntimeInfo {
            pid: 42,
            actual_port: 16800,
            rpc_secret: "secret".to_string(),
            rpc_endpoint: "http://127.0.0.1:16800/jsonrpc".to_string(),
            binary_source: Aria2BinarySource::Sidecar,
            sidecar_name: Some("aria2-next".to_string()),
            app_data_dir: Some("/tmp/motrix-fnos".to_string()),
            aria2_session_path: Some("/tmp/motrix-fnos/aria2/aria2.session".to_string()),
            aria2_log_path: Some("/tmp/motrix-fnos/aria2/aria2.log".to_string()),
            launch_args: Some(vec!["--enable-rpc=true".to_string()]),
        };

        write_aria2_runtime_record(&path, &runtime).expect("runtime should write");
        let restored = read_aria2_runtime_record(&path)
            .expect("runtime should read")
            .expect("runtime should exist");
        assert_eq!(restored, runtime);

        remove_aria2_runtime_record(&path).expect("runtime should remove");
        assert!(read_aria2_runtime_record(&path)
            .expect("missing runtime should read")
            .is_none());
    }
}
