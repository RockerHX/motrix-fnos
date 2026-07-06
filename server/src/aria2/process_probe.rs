use std::time::Duration;

#[cfg(unix)]
pub(crate) fn read_process_command_line(pid: u32) -> Result<String, String> {
    let output = std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "command="])
        .output()
        .map_err(|error| format!("读取进程命令行失败，PID {}：{}", pid, error))?;

    if !output.status.success() {
        return Err(format!(
            "读取进程命令行失败，PID {}：ps 退出状态 {}",
            pid, output.status
        ));
    }

    let command_line = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if command_line.is_empty() {
        return Err(format!("读取进程命令行失败，PID {}：结果为空", pid));
    }

    Ok(command_line)
}

#[cfg(windows)]
pub(crate) fn read_process_command_line(pid: u32) -> Result<String, String> {
    let query = format!(
        "(Get-CimInstance Win32_Process -Filter \"ProcessId = {}\").CommandLine",
        pid
    );
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &query])
        .output()
        .map_err(|error| format!("读取进程命令行失败，PID {}：{}", pid, error))?;

    if !output.status.success() {
        return Err(format!(
            "读取进程命令行失败，PID {}：PowerShell 退出状态 {}",
            pid, output.status
        ));
    }

    let command_line = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if command_line.is_empty() {
        return Err(format!("读取进程命令行失败，PID {}：结果为空", pid));
    }

    Ok(command_line)
}

#[cfg(unix)]
pub(crate) fn terminate_process(pid: u32) -> bool {
    let _ = std::process::Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status();
    if wait_until_process_exits(pid, Duration::from_millis(800)) {
        return true;
    }

    let _ = std::process::Command::new("kill")
        .arg("-KILL")
        .arg(pid.to_string())
        .status();
    wait_until_process_exits(pid, Duration::from_millis(800))
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
pub(crate) fn terminate_process(pid: u32) -> bool {
    let _ = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status();
    wait_until_process_exits(pid, Duration::from_millis(800))
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
