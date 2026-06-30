use crate::tasks::DownloadTask;
use std::process::Child;
use std::sync::atomic::AtomicU64;
use std::sync::Mutex;

pub struct AppState {
    pub aria2_process: Mutex<Option<Child>>,
    pub download_tasks: Mutex<Vec<DownloadTask>>,
    pub next_task_id: AtomicU64,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            aria2_process: Mutex::new(None),
            download_tasks: Mutex::new(Vec::new()),
            next_task_id: AtomicU64::new(1),
        }
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        if let Ok(process) = self.aria2_process.get_mut() {
            if let Some(mut child) = process.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}
