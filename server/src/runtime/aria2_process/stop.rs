use super::types::{Aria2ProcessStatus, ManagedAria2Process};
use crate::aria2::terminate_process;
use crate::debug_logs::DebugLogStore;
use std::sync::Mutex;
use std::time::Duration;

pub fn stop_process(
    process: &Mutex<Option<ManagedAria2Process>>,
    debug_logs: &DebugLogStore,
) -> Result<Aria2ProcessStatus, String> {
    let mut guard = process.lock().map_err(|_| {
        debug_logs.error("aria2", "无法写入 Aria2 进程状态");
        "无法写入 Aria2 进程状态".to_string()
    })?;

    if let Some(mut child) = guard.take() {
        let pid = child.id();
        if !child.is_running()? {
            debug_logs.warn(
                "aria2",
                format!("停止 Aria2 进程：PID {} 已不存在，清理本地句柄", pid),
            );
        } else {
            debug_logs.info("aria2", format!("准备停止 Aria2 进程，PID {}", pid));
            if let Err(error) = child.kill() {
                debug_logs.warn(
                    "aria2",
                    format!("{}，尝试按 PID 兜底终止，PID {}", error, pid),
                );
            }
            child.wait();
            if !wait_until_process_exits(pid, Duration::from_millis(800)) && !terminate_process(pid)
            {
                let error = format!("停止 Aria2 进程后 PID {} 仍然存活", pid);
                debug_logs.error("aria2", &error);
                return Err(error);
            }
            debug_logs.info("aria2", format!("Aria2 进程已停止，PID {}", pid));
        }
    } else {
        debug_logs.info("aria2", "停止 Aria2 进程：当前没有运行中的进程");
    }

    Ok(Aria2ProcessStatus {
        running: false,
        pid: None,
        binary_source: None,
        message: "Aria2 进程已停止".to_string(),
    })
}

#[cfg(unix)]
fn process_is_running(pid: u32) -> bool {
    std::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(unix)]
fn wait_until_process_exits(pid: u32, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if !process_is_running(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    !process_is_running(pid)
}

#[cfg(windows)]
fn process_is_running(pid: u32) -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/NH"])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
        .unwrap_or(false)
}

#[cfg(windows)]
fn wait_until_process_exits(pid: u32, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if !process_is_running(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    !process_is_running(pid)
}
