use crate::database::AppDatabase;
use crate::debug_logs::DebugLogStore;
use crate::tasks::DownloadTask;
use std::collections::HashSet;
use std::process::Child;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Mutex;
use tauri_plugin_shell::process::CommandChild;

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
}

impl Drop for AppState {
    fn drop(&mut self) {
        if let Ok(process) = self.aria2_process.get_mut() {
            if let Some(child) = process.take() {
                let _ = child.kill();
            }
        }
    }
}
