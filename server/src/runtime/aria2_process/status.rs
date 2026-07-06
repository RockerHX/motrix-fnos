use super::types::{Aria2ProcessStatus, ManagedAria2Process};
use std::sync::Mutex;

pub fn process_status(
    process: &Mutex<Option<ManagedAria2Process>>,
) -> Result<Aria2ProcessStatus, String> {
    let mut guard = process
        .lock()
        .map_err(|_| "无法读取 Aria2 进程状态".to_string())?;

    let Some(child) = guard.as_mut() else {
        return Ok(Aria2ProcessStatus {
            running: false,
            pid: None,
            binary_source: None,
            message: "Aria2 进程未启动".to_string(),
        });
    };

    if child.is_running()? {
        return Ok(Aria2ProcessStatus {
            running: true,
            pid: Some(child.id()),
            binary_source: Some(child.source()),
            message: "Aria2 进程已启动".to_string(),
        });
    }

    let pid = child.id();
    let source = child.source();
    let _ = guard.take();
    Ok(Aria2ProcessStatus {
        running: false,
        pid: Some(pid),
        binary_source: Some(source),
        message: format!("Aria2 进程已退出，PID {}", pid),
    })
}
