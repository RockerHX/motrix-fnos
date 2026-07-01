use crate::config::aria2::{Aria2BinarySource, Aria2Config};
use crate::database::AppDatabase;
use crate::debug_logs::DebugLogStore;
use crate::tasks::DownloadTask;
use std::collections::HashSet;
use std::process::Child;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Mutex;
use tauri_plugin_shell::process::CommandChild;


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aria2RuntimeInfo {
    pub pid: u32,
    pub actual_port: u16,
    pub rpc_secret: String,
    pub rpc_endpoint: String,
    pub binary_source: Aria2BinarySource,
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
        let state = Self {
            aria2_process: Mutex::new(None),
            aria2_runtime: Mutex::new(None),
            download_tasks: Mutex::new(download_tasks),
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
        self.aria2_runtime.lock().ok().and_then(|runtime| runtime.clone())
    }

    pub fn set_aria2_runtime(&self, runtime: Aria2RuntimeInfo) -> Result<(), String> {
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
    }

    pub fn aria2_config(&self) -> Aria2Config {
        let mut config = Aria2Config::from_env();
        if let Some(runtime) = self.aria2_runtime_snapshot() {
            config.rpc_port = runtime.actual_port;
            config.rpc_secret = runtime.rpc_secret;
        }
        config
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        if let Ok(runtime) = self.aria2_runtime.get_mut() {
            *runtime = None;
        }
        if let Ok(process) = self.aria2_process.get_mut() {
            if let Some(child) = process.take() {
                let _ = child.kill();
            }
        }
    }
}
