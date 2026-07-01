use crate::config::aria2::{Aria2BinarySource, Aria2Config};
use std::process::Child;
use std::sync::Mutex;
use tauri_plugin_shell::process::CommandChild;

pub use motrix_fnos_server::state::{
    app_data_dir_from_database_path, aria2_runtime_path, aria2_runtime_paths,
    read_aria2_runtime_record, remove_aria2_runtime_record, write_aria2_runtime_record,
    Aria2RuntimeInfo, Aria2RuntimePaths, ServerState, ARIA2_LOG_FILE_NAME,
    ARIA2_RUNTIME_DIR_NAME, ARIA2_RUNTIME_FILE_NAME, ARIA2_SESSION_FILE_NAME,
};

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
    pub core: ServerState,
    pub aria2_process: Mutex<Option<ManagedAria2Process>>,
}

impl AppState {
    pub fn new(
        database: motrix_fnos_server::database::AppDatabase,
        download_tasks: Vec<motrix_fnos_server::tasks::DownloadTask>,
        next_task_id: u64,
    ) -> Self {
        Self {
            core: ServerState::new(database, download_tasks, next_task_id),
            aria2_process: Mutex::new(None),
        }
    }

    pub fn aria2_runtime_snapshot(&self) -> Option<Aria2RuntimeInfo> {
        self.core.aria2_runtime_snapshot()
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

    pub fn aria2_config(&self) -> Aria2Config {
        self.core.aria2_config()
    }

    pub fn with_aria2_runtime_paths(&self, config: Aria2Config) -> Result<Aria2Config, String> {
        self.core.with_aria2_runtime_paths(config)
    }

    pub fn build_aria2_runtime_info(
        &self,
        pid: u32,
        config: &Aria2Config,
        source: Aria2BinarySource,
        launch_args: Vec<String>,
    ) -> Aria2RuntimeInfo {
        self.core
            .build_aria2_runtime_info(pid, config, source, launch_args)
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        if let Ok(runtime) = self.core.aria2_runtime.get_mut() {
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
