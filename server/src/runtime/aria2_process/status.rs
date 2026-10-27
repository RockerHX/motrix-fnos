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
    // 正常停止的退出结果由生命周期协调器确认，状态查询不能先丢弃。
    if !child.stop_requested {
        let _ = guard.take();
    }
    Ok(Aria2ProcessStatus {
        running: false,
        pid: Some(pid),
        binary_source: Some(source),
        message: format!("Aria2 进程已退出，PID {}", pid),
    })
}
