use crate::config::aria2::{Aria2BinarySource, Aria2Config};
use crate::database::AppDatabase;
use crate::debug_logs::DebugLogStore;
use crate::tasks::{DownloadTask, TaskMemoryState};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

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
    pub aria2_state_path: Option<String>,
    #[serde(default)]
    pub aria2_log_path: Option<String>,
    #[serde(default)]
    pub launch_args: Option<Vec<String>>,
    #[serde(default)]
    pub process_start_time: Option<u64>,
    #[serde(default)]
    pub process_uid: Option<u32>,
}

pub const ARIA2_RUNTIME_FILE_NAME: &str = "aria2-runtime.json";
pub const ARIA2_RUNTIME_DIR_NAME: &str = "aria2";
pub const ARIA2_SESSION_FILE_NAME: &str = "aria2.session";
pub const ARIA2_LOG_FILE_NAME: &str = "aria2.log";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aria2RuntimePaths {
    pub runtime_dir: PathBuf,
    pub session_path: PathBuf,
    pub state_dir: PathBuf,
    pub log_path: PathBuf,
}

#[derive(Debug, Default)]
pub struct ShutdownState {
    exiting: AtomicBool,
}

impl ShutdownState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_exiting(&self) -> bool {
        self.exiting.load(Ordering::SeqCst)
    }

    pub fn begin_shutdown(&self) -> bool {
        !self.exiting.swap(true, Ordering::SeqCst)
    }

    pub fn mark_exiting(&self) {
        self.exiting.store(true, Ordering::SeqCst);
    }
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

pub fn native_aria2_runtime_paths(app_data_dir: &Path) -> Aria2RuntimePaths {
    let runtime_dir = app_data_dir.join("engine/native-v2");
    Aria2RuntimePaths {
        session_path: runtime_dir.join("session").join(ARIA2_SESSION_FILE_NAME),
        state_dir: runtime_dir.join("state"),
        log_path: app_data_dir
            .join(ARIA2_RUNTIME_DIR_NAME)
            .join(ARIA2_LOG_FILE_NAME),
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
    let temporary =
        path.with_extension(format!("json.tmp-{}", crate::aria2::generate_rpc_secret()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&temporary)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|error: std::io::Error| format!("写入 Aria2 运行态文件失败：{error}"))
}

pub fn read_aria2_runtime_record(path: &Path) -> Result<Option<Aria2RuntimeInfo>, String> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("读取 Aria2 运行态元数据失败：{error}")),
        Ok(metadata)
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.nlink() != 1 =>
        {
            return Err("Aria2 运行态不是普通文件，已保留".to_string());
        }
        Ok(_) => {}
    }
    let content = fs::read_to_string(path)
        .map_err(|error| format!("读取 Aria2 运行态文件失败：{}（{}）", path.display(), error))?;
    serde_json::from_str(&content)
        .map(Some)
        .map_err(|error| format!("解析 Aria2 运行态文件失败：{}", error))
}

fn ensure_aria2_session_file(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata)
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.nlink() != 1 =>
        {
            return Err("Aria2 session 不是普通文件，已保留".to_string());
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            return Err(format!("读取 Aria2 session 失败：{error}"))
        }
        _ => {}
    }

    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
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

pub(crate) fn ensure_private_directory(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    let mut path =
        fs::canonicalize(root).map_err(|error| format!("读取应用私有目录失败：{error}"))?;
    for component in relative.components() {
        let std::path::Component::Normal(name) = component else {
            return Err("私有目录必须位于应用数据目录内".to_string());
        };
        path.push(name);
        match fs::symlink_metadata(&path) {
            Ok(metadata)
                if !metadata.is_dir()
                    || metadata.file_type().is_symlink()
                    || metadata.uid() != unsafe { libc::geteuid() } =>
            {
                return Err("私有状态目录不是普通目录，已保留".to_string());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&path).map_err(|error| format!("创建私有状态目录失败：{error}"))?;
            }
            Err(error) => return Err(format!("读取私有状态目录失败：{error}")),
        }
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("设置私有状态目录权限失败：{error}"))?;
    }
    Ok(path)
}

fn validate_native_state_tree(root: &Path) -> Result<(), String> {
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in
            fs::read_dir(directory).map_err(|error| format!("读取原生状态目录失败：{error}"))?
        {
            let path = entry
                .map_err(|error| format!("读取原生状态条目失败：{error}"))?
                .path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("读取原生状态文件失败：{error}"))?;
            if metadata.file_type().is_symlink()
                || metadata.uid() != unsafe { libc::geteuid() }
                || (!metadata.is_dir() && (!metadata.is_file() || metadata.nlink() != 1))
            {
                return Err("原生状态含有无法证明归属的文件，已保留并拒绝加载".to_string());
            }
            if metadata.is_dir() {
                directories.push(path);
            }
        }
    }
    Ok(())
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

pub struct ServerState {
    pub aria2_runtime: Mutex<Option<Aria2RuntimeInfo>>,
    pub download_tasks: TaskMemoryState,
    pub database: AppDatabase,
    pub app_data_dir: PathBuf,
    pub aria2_runtime_path: PathBuf,
    pub debug_logs: DebugLogStore,
    pub next_task_id: AtomicU64,
    pub notified_task_events: Mutex<HashSet<String>>,
    pub shutdown: ShutdownState,
}

impl ServerState {
    pub fn new(
        database: AppDatabase,
        download_tasks: Vec<DownloadTask>,
        next_task_id: u64,
    ) -> Self {
        let restored_count = download_tasks.len();
        let app_data_dir = app_data_dir_from_database_path(&database.path);
        let aria2_runtime_path = aria2_runtime_path(&database.path);
        let state = Self {
            aria2_runtime: Mutex::new(None),
            download_tasks: TaskMemoryState::new(download_tasks),
            app_data_dir,
            aria2_runtime_path,
            database,
            debug_logs: DebugLogStore::default(),
            next_task_id: AtomicU64::new(next_task_id),
            notified_task_events: Mutex::new(HashSet::new()),
            shutdown: ShutdownState::new(),
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

    pub fn clear_aria2_runtime(&self) -> Result<(), String> {
        remove_aria2_runtime_record(&self.aria2_runtime_path)?;
        *self
            .aria2_runtime
            .lock()
            .map_err(|_| "无法清除 Aria2 运行态".to_string())? = None;
        Ok(())
    }

    pub fn load_saved_aria2_runtime(&self) -> Result<Option<Aria2RuntimeInfo>, String> {
        read_aria2_runtime_record(&self.aria2_runtime_path)
    }

    pub fn aria2_config(&self) -> Aria2Config {
        let mut config = Aria2Config::from_env();
        let paths = native_aria2_runtime_paths(&self.app_data_dir);
        config.state_dir = Some(paths.state_dir.display().to_string());
        config.session_path = Some(paths.session_path.display().to_string());
        if let Some(runtime) = self.aria2_runtime_snapshot() {
            config.rpc_port = runtime.actual_port;
            config.rpc_secret = runtime.rpc_secret;
            config.log_path = runtime.aria2_log_path.clone();
        }
        config
    }

    pub fn with_aria2_runtime_paths(&self, mut config: Aria2Config) -> Result<Aria2Config, String> {
        let paths = native_aria2_runtime_paths(&self.app_data_dir);
        for directory in [
            paths.session_path.parent().unwrap(),
            paths.log_path.parent().unwrap(),
        ] {
            ensure_private_directory(
                &self.app_data_dir,
                directory
                    .strip_prefix(&self.app_data_dir)
                    .map_err(|_| "Aria2 目录不属于应用数据目录".to_string())?,
            )?;
        }
        ensure_private_directory(&self.app_data_dir, Path::new("engine/native-v2/state"))?;
        validate_native_state_tree(&paths.state_dir)?;
        config.state_dir = Some(paths.state_dir.display().to_string());
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
            aria2_state_path: config.state_dir.clone(),
            aria2_log_path: config.log_path.clone(),
            launch_args: Some(launch_args),
            process_start_time: read_process_start_time(pid),
            process_uid: read_process_uid(pid),
        }
    }
}

fn read_process_start_time(pid: u32) -> Option<u64> {
    let content = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let command_end = content.rfind(") ")?;
    content[command_end + 2..]
        .split_whitespace()
        .nth(19)
        .and_then(|value| value.parse().ok())
}

fn read_process_uid(pid: u32) -> Option<u32> {
    let content = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    content
        .lines()
        .find(|line| line.starts_with("Uid:"))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse().ok())
}

#[cfg(test)]
mod tests;
