use crate::config::aria2::{Aria2Config, ARIA2_PATH_ENV};
use crate::database::{
    connect_database,
    tasks::{list_download_tasks, max_download_task_id, persist_download_task_states},
    DATABASE_FILE_NAME,
};
use crate::runtime::ManagedAria2Process;
use crate::state::{Aria2RuntimeInfo, ServerState};
use crate::tasks::DownloadTask;
use crate::tasks::{is_pending_magnet_metadata_task, DownloadTaskStatus};
use serde::Serialize;
use std::env;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::sync::broadcast;

pub const APP_DATA_DIR_ENV: &str = "MOTRIX_FNOS_APP_DATA_DIR";
pub const HTTP_ADDR_ENV: &str = "MOTRIX_FNOS_HTTP_ADDR";
pub const ACCESSIBLE_PATHS_FILE_ENV: &str = "MOTRIX_FNOS_ACCESSIBLE_PATHS_FILE";
pub const DEFAULT_HTTP_ADDR: &str = "127.0.0.1:17080";
pub const ACCESSIBLE_PATHS_FILE_NAME: &str = "accessible-paths.json";
const RUNTIME_EVENT_BUFFER: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerRuntimeConfig {
    pub app_data_dir: PathBuf,
    pub database_path: PathBuf,
    pub http_addr: SocketAddr,
    pub aria2_path: Option<PathBuf>,
    pub accessible_paths_path: PathBuf,
}

impl ServerRuntimeConfig {
    pub fn from_env() -> Result<Self, String> {
        let app_data_dir = env::var(APP_DATA_DIR_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(default_local_app_data_dir);
        let http_addr = env::var(HTTP_ADDR_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_HTTP_ADDR.to_string())
            .parse::<SocketAddr>()
            .map_err(|error| format!("解析 HTTP 监听地址失败：{}", error))?;
        let aria2_path = env::var(ARIA2_PATH_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from);
        let database_path = app_data_dir.join(DATABASE_FILE_NAME);
        let accessible_paths_path = env::var(ACCESSIBLE_PATHS_FILE_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| app_data_dir.join(ACCESSIBLE_PATHS_FILE_NAME));
        Ok(Self {
            app_data_dir,
            database_path,
            http_addr,
            aria2_path,
            accessible_paths_path,
        })
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeExitingPayload {
    pub reason: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TasksSnapshotPayload {
    pub tasks: Vec<DownloadTask>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEvent {
    TasksSnapshot(TasksSnapshotPayload),
    RuntimeExiting(RuntimeExitingPayload),
}

#[derive(Clone)]
pub struct RuntimeEventHub {
    sender: broadcast::Sender<RuntimeEvent>,
}

impl RuntimeEventHub {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(RUNTIME_EVENT_BUFFER);
        Self { sender }
    }

    pub fn send(&self, event: RuntimeEvent) -> Result<usize, String> {
        match self.sender.send(event) {
            Ok(count) => Ok(count),
            // 没有前端订阅者只表示当前无需投递事件，不能因此把正常的后台任务流程判为失败。
            Err(_) => Ok(0),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<RuntimeEvent> {
        self.sender.subscribe()
    }
}

impl Default for RuntimeEventHub {
    fn default() -> Self {
        Self::new()
    }
}

pub struct HttpAppState {
    pub core: Arc<ServerState>,
    pub runtime: ServerRuntimeConfig,
    pub base_aria2_config: Aria2Config,
    pub aria2_process: Mutex<Option<ManagedAria2Process>>,
    pub runtime_events: RuntimeEventHub,
}

impl HttpAppState {
    pub fn new(core: ServerState, runtime: ServerRuntimeConfig) -> Self {
        let mut base_aria2_config = Aria2Config::from_env();
        base_aria2_config.aria2_path = runtime
            .aria2_path
            .as_ref()
            .map(|path| path.display().to_string());

        Self {
            core: Arc::new(core),
            runtime,
            base_aria2_config,
            aria2_process: Mutex::new(None),
            runtime_events: RuntimeEventHub::new(),
        }
    }

    pub fn aria2_runtime_snapshot(&self) -> Option<Aria2RuntimeInfo> {
        self.core.aria2_runtime_snapshot()
    }

    pub fn aria2_config(&self) -> Aria2Config {
        let mut config = self.base_aria2_config.clone();
        if let Some(runtime) = self.aria2_runtime_snapshot() {
            config.rpc_port = runtime.actual_port;
            config.rpc_secret = runtime.rpc_secret;
            config.session_path = runtime.aria2_session_path.clone();
            config.log_path = runtime.aria2_log_path.clone();
        }
        config
    }

    pub fn with_aria2_runtime_paths(&self, config: Aria2Config) -> Result<Aria2Config, String> {
        self.core.with_aria2_runtime_paths(config)
    }

    pub fn build_aria2_runtime_info(
        &self,
        pid: u32,
        config: &Aria2Config,
        source: crate::config::aria2::Aria2BinarySource,
        launch_args: Vec<String>,
    ) -> Aria2RuntimeInfo {
        self.core
            .build_aria2_runtime_info(pid, config, source, launch_args)
    }

    pub fn set_aria2_runtime(&self, runtime: Aria2RuntimeInfo) -> Result<(), String> {
        self.core.set_aria2_runtime(runtime)
    }

    pub fn clear_aria2_runtime(&self) {
        self.core.clear_aria2_runtime()
    }

    pub fn load_saved_aria2_runtime(&self) -> Option<Aria2RuntimeInfo> {
        self.core.load_saved_aria2_runtime()
    }

    pub fn request_shutdown(&self, reason: impl Into<String>) {
        let reason = reason.into();
        if !self.core.shutdown.begin_shutdown() {
            self.core
                .debug_logs
                .info("runtime.exit", "服务退出流程已在执行，忽略重复退出请求");
            return;
        }

        self.core.debug_logs.info("runtime.exit", &reason);
        let _ = self
            .runtime_events
            .send(RuntimeEvent::RuntimeExiting(RuntimeExitingPayload {
                reason,
                timestamp: current_timestamp_ms(),
            }));
    }
}

pub async fn bootstrap_http_app_state(
    runtime: &ServerRuntimeConfig,
) -> Result<Arc<HttpAppState>, String> {
    let database = connect_database(runtime.database_path.clone()).await?;
    let mut restored_tasks = list_download_tasks(&database.pool).await?;
    // 必须先用应用私有 metadata 目录对账恢复任务，再持久化修正后的状态，避免丢失目录在下次启动时继续伪装成可恢复任务。
    reconcile_magnet_metadata_dirs(&runtime.app_data_dir, &mut restored_tasks)?;
    persist_download_task_states(&database.pool, &restored_tasks).await?;
    let next_task_id = max_download_task_id(&database.pool)
        .await?
        .saturating_add(1);
    let state = ServerState::new(database, restored_tasks, next_task_id);

    Ok(Arc::new(HttpAppState::new(state, runtime.clone())))
}

fn reconcile_magnet_metadata_dirs(
    app_data_dir: &Path,
    tasks: &mut [DownloadTask],
) -> Result<(), String> {
    let metadata_root = app_data_dir.join("magnet-metadata");
    if !metadata_root.exists() {
        return Ok(());
    }
    if !metadata_root.is_dir() {
        return Err(format!(
            "磁链 metadata 根目录不是文件夹：{}",
            metadata_root.display()
        ));
    }

    // 清理范围固定在应用私有根目录；先收集仍被任务引用的目录，随后只删除其中未被引用的孤儿子目录。
    let mut referenced_dirs = std::collections::BTreeSet::new();
    for task in tasks.iter_mut() {
        let pending_metadata_dir = if is_pending_magnet_metadata_task(task) {
            Some(magnet_metadata_task_dir(app_data_dir, task.id))
        } else {
            None
        };

        if task.confirmation_required {
            let metadata_missing = task
                .metadata_torrent_path
                .as_deref()
                .filter(|path| !path.trim().is_empty())
                .map(PathBuf::from)
                .filter(|path| path.is_file())
                .is_none();
            if metadata_missing {
                task.status = DownloadTaskStatus::Error;
                task.gid = None;
                task.confirmation_required = false;
                task.download_speed = 0;
                task.error_code = None;
                task.error_message = Some("磁链 metadata 文件丢失，请重新添加磁链".to_string());
                task.metadata_torrent_path = None;
            }
        }

        if let Some(metadata_dir) = pending_metadata_dir {
            if metadata_dir.is_dir() {
                referenced_dirs.insert(metadata_dir);
            } else {
                task.status = DownloadTaskStatus::Error;
                task.gid = None;
                task.download_speed = 0;
                task.error_code = None;
                task.error_message = Some("磁链 metadata 临时目录丢失，请重新添加磁链".to_string());
            }
        } else if let Some(metadata_dir) = task
            .metadata_torrent_path
            .as_deref()
            .and_then(|path| Path::new(path).parent())
        {
            referenced_dirs.insert(metadata_dir.to_path_buf());
        }
    }

    for entry in fs::read_dir(&metadata_root).map_err(|error| {
        format!(
            "读取磁链 metadata 根目录失败：{}（{}）",
            metadata_root.display(),
            error
        )
    })? {
        let entry = entry.map_err(|error| {
            format!(
                "读取磁链 metadata 目录项失败：{}（{}）",
                metadata_root.display(),
                error
            )
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if referenced_dirs.contains(&path) {
            continue;
        }
        fs::remove_dir_all(&path).map_err(|error| {
            format!(
                "清理孤儿磁链 metadata 目录失败：{}（{}）",
                path.display(),
                error
            )
        })?;
    }

    Ok(())
}

fn magnet_metadata_task_dir(app_data_dir: &Path, task_id: u64) -> PathBuf {
    app_data_dir
        .join("magnet-metadata")
        .join(format!("task-{task_id}"))
}

pub async fn run_server() -> Result<(), String> {
    let runtime = ServerRuntimeConfig::from_env()?;
    let state = bootstrap_http_app_state(&runtime).await?;
    crate::runtime::spawn_task_monitor(state.clone());

    let router = crate::api::management_router(state.clone());
    let listener = TcpListener::bind(state.runtime.http_addr)
        .await
        .map_err(|error| {
            format!(
                "绑定 HTTP 监听地址失败：{}（{}）",
                state.runtime.http_addr, error
            )
        })?;
    state.core.debug_logs.info(
        "app",
        format!(
            "独立 server 入口已初始化，监听地址 {}，数据目录 {}",
            state.runtime.http_addr,
            state.runtime.app_data_dir.display()
        ),
    );
    axum::serve(listener, router)
        .with_graceful_shutdown(wait_for_shutdown_signal(state.clone()))
        .await
        .map_err(|error| format!("HTTP 服务运行失败：{}", error))
}

async fn wait_for_shutdown_signal(state: Arc<HttpAppState>) {
    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            state.request_shutdown("收到停止信号");
            crate::runtime::run_shutdown_cleanup(&state).await;
        }
        Err(error) => state
            .core
            .debug_logs
            .error("runtime.exit", format!("等待停止信号失败：{}", error)),
    }
}

fn default_local_app_data_dir() -> PathBuf {
    if cfg!(target_os = "macos") {
        return home_dir_fallback()
            .join("Library")
            .join("Application Support")
            .join("motrix-fnos");
    }

    if let Some(path) = env::var_os("XDG_DATA_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    {
        return path.join("motrix-fnos");
    }

    home_dir_fallback()
        .join(".local")
        .join("share")
        .join("motrix-fnos")
}

fn home_dir_fallback() -> PathBuf {
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
