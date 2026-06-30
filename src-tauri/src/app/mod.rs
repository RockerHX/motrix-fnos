use std::process::Child;
use std::sync::Mutex;

#[derive(Default)]
pub struct AppState {
    pub aria2_process: Mutex<Option<Child>>,
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
