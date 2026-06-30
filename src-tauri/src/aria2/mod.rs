use crate::config::aria2::Aria2Config;
use serde::Serialize;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Aria2ConfigStatus {
    pub configured: bool,
    pub path: Option<String>,
    pub path_exists: bool,
    pub rpc_host: String,
    pub rpc_port: u16,
    pub rpc_secret_configured: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Aria2ProcessStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub message: String,
}

impl Aria2ConfigStatus {
    pub fn from_config(config: &Aria2Config) -> Self {
        let path_exists = config
            .aria2_path
            .as_deref()
            .map(|path| Path::new(path).is_file())
            .unwrap_or(false);

        Self {
            configured: config.aria2_path.is_some(),
            path: config.aria2_path.clone(),
            path_exists,
            rpc_host: config.rpc_host.clone(),
            rpc_port: config.rpc_port,
            rpc_secret_configured: !config.rpc_secret.is_empty(),
        }
    }
}

pub fn process_status(process: &Mutex<Option<Child>>) -> Result<Aria2ProcessStatus, String> {
    let guard = process
        .lock()
        .map_err(|_| "无法读取 Aria2 进程状态".to_string())?;

    Ok(match guard.as_ref() {
        Some(child) => Aria2ProcessStatus {
            running: true,
            pid: Some(child.id()),
            message: "Aria2 进程已启动".to_string(),
        },
        None => Aria2ProcessStatus {
            running: false,
            pid: None,
            message: "Aria2 进程未启动".to_string(),
        },
    })
}

pub fn start_process(
    process: &Mutex<Option<Child>>,
    config: &Aria2Config,
) -> Result<Aria2ProcessStatus, String> {
    let mut guard = process
        .lock()
        .map_err(|_| "无法写入 Aria2 进程状态".to_string())?;

    if let Some(child) = guard.as_ref() {
        return Ok(Aria2ProcessStatus {
            running: true,
            pid: Some(child.id()),
            message: "Aria2 进程已在运行".to_string(),
        });
    }

    let aria2_path = config
        .aria2_path
        .as_deref()
        .ok_or_else(|| "未配置 Aria2 Next 路径，请设置 MOTRIX_FNOS_ARIA2_PATH".to_string())?;

    if !Path::new(aria2_path).is_file() {
        return Err(format!("Aria2 Next 路径不存在或不是文件：{}", aria2_path));
    }

    let mut command = Command::new(aria2_path);
    command
        .arg("--enable-rpc=true")
        .arg(format!("--rpc-listen-port={}", config.rpc_port))
        .arg(format!("--rpc-listen-all=false"))
        .arg("--continue=true")
        .arg("--console-log-level=warn")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    if !config.rpc_secret.is_empty() {
        command.arg(format!("--rpc-secret={}", config.rpc_secret));
    }

    let child = command
        .spawn()
        .map_err(|error| format!("启动 Aria2 Next 失败：{}", error))?;
    let pid = child.id();
    *guard = Some(child);

    Ok(Aria2ProcessStatus {
        running: true,
        pid: Some(pid),
        message: "Aria2 进程启动成功".to_string(),
    })
}

pub fn stop_process(process: &Mutex<Option<Child>>) -> Result<Aria2ProcessStatus, String> {
    let mut guard = process
        .lock()
        .map_err(|_| "无法写入 Aria2 进程状态".to_string())?;

    if let Some(mut child) = guard.take() {
        child
            .kill()
            .map_err(|error| format!("停止 Aria2 进程失败：{}", error))?;
        let _ = child.wait();
    }

    Ok(Aria2ProcessStatus {
        running: false,
        pid: None,
        message: "Aria2 进程已停止".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(path: Option<&str>) -> Aria2Config {
        Aria2Config {
            aria2_path: path.map(ToOwned::to_owned),
            rpc_host: "127.0.0.1".to_string(),
            rpc_port: 6800,
            rpc_secret: String::new(),
        }
    }

    #[test]
    fn start_process_returns_clear_error_without_path() {
        let process = Mutex::new(None);
        let error =
            start_process(&process, &test_config(None)).expect_err("missing path should fail");

        assert!(error.contains("MOTRIX_FNOS_ARIA2_PATH"));
    }

    #[test]
    fn start_process_returns_clear_error_for_invalid_path() {
        let process = Mutex::new(None);
        let error = start_process(&process, &test_config(Some("/definitely/missing/aria2")))
            .expect_err("invalid path should fail");

        assert!(error.contains("路径不存在"));
    }
}
