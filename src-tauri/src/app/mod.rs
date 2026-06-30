use crate::database::AppDatabase;
use crate::debug_logs::DebugLogStore;
use crate::tasks::DownloadTask;
use std::process::Child;
use std::sync::atomic::AtomicU64;
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
}

impl AppState {
    pub fn new(database: AppDatabase) -> Self {
        let state = Self {
            aria2_process: Mutex::new(None),
            download_tasks: Mutex::new(Vec::new()),
            database,
            debug_logs: DebugLogStore::default(),
            next_task_id: AtomicU64::new(1),
        };
        state.debug_logs.info("app", "应用启动，调试日志队列已初始化");
        state.debug_logs.info(
            "database",
            format!("SQLite 数据库已初始化：{}", state.database.path.display()),
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
