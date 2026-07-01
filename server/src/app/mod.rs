use crate::config::aria2::{Aria2Config, ARIA2_PATH_ENV};
use crate::database::{connect_database, tasks::list_download_tasks, tasks::max_download_task_id, DATABASE_FILE_NAME};
use crate::runtime::ManagedAria2Process;
use crate::state::{Aria2RuntimeInfo, ServerState};
use serde::Serialize;
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

#[cfg(test)]
use std::sync::OnceLock;

pub const APP_DATA_DIR_ENV: &str = "MOTRIX_FNOS_APP_DATA_DIR";
pub const HTTP_ADDR_ENV: &str = "MOTRIX_FNOS_HTTP_ADDR";
pub const DEFAULT_HTTP_ADDR: &str = "127.0.0.1:17080";
const RUNTIME_EVENT_BUFFER: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerRuntimeConfig {
    pub app_data_dir: PathBuf,
    pub database_path: PathBuf,
    pub http_addr: SocketAddr,
    pub aria2_path: Option<PathBuf>,
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

        Ok(Self {
            app_data_dir,
            database_path,
            http_addr,
            aria2_path,
        })
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeExitingPayload {
    pub reason: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEvent {
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
        self.sender
            .send(event)
            .map_err(|error| format!("发送运行时事件失败：{}", error))
    }

    pub fn subscribe(&self) -> broadcast::Receiver<RuntimeEvent> {
        self.sender.subscribe()
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
        if self.core.is_exiting.swap(true, Ordering::SeqCst) {
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
    let restored_tasks = list_download_tasks(&database.pool).await?;
    let next_task_id = max_download_task_id(&database.pool).await?.saturating_add(1);
    let state = ServerState::new(database, restored_tasks, next_task_id);

    Ok(Arc::new(HttpAppState::new(state, runtime.clone())))
}

pub async fn run_server() -> Result<(), String> {
    let runtime = ServerRuntimeConfig::from_env()?;
    let state = bootstrap_http_app_state(&runtime).await?;
    state.core.debug_logs.info(
        "app",
        format!(
            "独立 server 入口已初始化，监听地址 {}，数据目录 {}",
            state.runtime.http_addr,
            state.runtime.app_data_dir.display()
        ),
    );

    tokio::signal::ctrl_c().await.map_err(|error| {
        format!(
            "等待停止信号失败，监听地址 {}：{}",
            state.runtime.http_addr, error
        )
    })?;
    state.request_shutdown("收到停止信号");
    Ok(())
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

    home_dir_fallback().join(".local").join("share").join("motrix-fnos")
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
mod tests {
    use super::*;
    use crate::database::tasks::upsert_download_task;
    use crate::tasks::{DownloadTask, DownloadTaskStatus};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn runtime_config_uses_explicit_env_values() {
        let _guard = env_lock().lock().expect("env lock should succeed");
        let temp_dir = std::env::temp_dir().join(format!("motrix-fnos-app-config-{}", now_ms()));
        let aria2_path = temp_dir.join("aria2-next");

        std::env::set_var(APP_DATA_DIR_ENV, &temp_dir);
        std::env::set_var(HTTP_ADDR_ENV, "127.0.0.1:18080");
        std::env::set_var(ARIA2_PATH_ENV, &aria2_path);

        let config = ServerRuntimeConfig::from_env().expect("config should load");

        assert_eq!(config.app_data_dir, temp_dir);
        assert_eq!(config.database_path, config.app_data_dir.join(DATABASE_FILE_NAME));
        assert_eq!(config.http_addr.to_string(), "127.0.0.1:18080");
        assert_eq!(config.aria2_path.as_deref(), Some(aria2_path.as_path()));

        std::env::remove_var(APP_DATA_DIR_ENV);
        std::env::remove_var(HTTP_ADDR_ENV);
        std::env::remove_var(ARIA2_PATH_ENV);
    }

    #[test]
    fn bootstrap_http_app_state_restores_database_state() {
        tokio::runtime::Runtime::new()
            .expect("tokio runtime should create")
            .block_on(async {
                let app_data_dir =
                    std::env::temp_dir().join(format!("motrix-fnos-http-app-state-{}", now_ms()));
                let runtime = ServerRuntimeConfig {
                    database_path: app_data_dir.join(DATABASE_FILE_NAME),
                    app_data_dir: app_data_dir.clone(),
                    http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
                    aria2_path: None,
                };

                let database = connect_database(runtime.database_path.clone())
                    .await
                    .expect("database should connect");
                let task = sample_task();
                upsert_download_task(&database.pool, &task)
                    .await
                    .expect("task should persist");
                database.pool.close().await;

                let state = bootstrap_http_app_state(&runtime)
                    .await
                    .expect("state should bootstrap");

                let tasks = state
                    .core
                    .download_tasks
                    .lock()
                    .expect("tasks should lock")
                    .clone();

                assert_eq!(state.runtime.app_data_dir, app_data_dir);
                assert_eq!(state.runtime.http_addr.to_string(), DEFAULT_HTTP_ADDR);
                assert_eq!(tasks.len(), 1);
                assert_eq!(tasks[0].id, task.id);
                assert_eq!(state.core.next_task_id.load(Ordering::SeqCst), task.id + 1);

                state.core.database.pool.close().await;
                let _ = std::fs::remove_file(&runtime.database_path);
                let _ = std::fs::remove_dir_all(&runtime.app_data_dir);
            });
    }

    #[test]
    fn request_shutdown_marks_exiting_and_broadcasts_event() {
        let temp_dir = std::env::temp_dir().join(format!("motrix-fnos-shutdown-{}", now_ms()));
        let runtime = ServerRuntimeConfig {
            database_path: temp_dir.join(DATABASE_FILE_NAME),
            app_data_dir: temp_dir,
            http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
            aria2_path: None,
        };
        let database = tokio::runtime::Runtime::new()
            .expect("tokio runtime should create")
            .block_on(async { connect_database(runtime.database_path.clone()).await })
            .expect("database should connect");
        let state = HttpAppState::new(ServerState::new(database, Vec::new(), 1), runtime);
        let mut receiver = state.runtime_events.subscribe();

        state.request_shutdown("收到停止信号");

        assert!(state.core.is_exiting.load(Ordering::SeqCst));
        let event = receiver.try_recv().expect("event should be broadcast");
        assert_eq!(
            event,
            RuntimeEvent::RuntimeExiting(RuntimeExitingPayload {
                reason: "收到停止信号".to_string(),
                timestamp: current_timestamp_ms(),
            })
        );
    }

    fn sample_task() -> DownloadTask {
        DownloadTask {
            id: 7,
            url: "https://example.com/archive.zip".to_string(),
            file_name: "archive.zip".to_string(),
            save_dir: "/downloads".to_string(),
            gid: Some("gid-7".to_string()),
            status: DownloadTaskStatus::Paused,
            total_length: 1024,
            completed_length: 512,
            download_speed: 0,
            error_code: None,
            error_message: None,
            file_path: Some("/downloads/archive.zip".to_string()),
            created_at: 100,
            updated_at: 101,
        }
    }

    fn now_ms() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be valid")
            .as_millis()
    }
}
