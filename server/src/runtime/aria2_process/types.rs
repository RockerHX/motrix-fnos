use crate::config::aria2::Aria2BinarySource;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Child;
use std::time::{Duration, Instant};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Aria2ProcessStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub binary_source: Option<Aria2BinarySource>,
    pub message: String,
}

#[derive(Debug)]
pub struct ManagedAria2Process {
    child: Child,
    source: Aria2BinarySource,
}

impl ManagedAria2Process {
    pub fn new(child: Child, source: Aria2BinarySource) -> Self {
        Self { child, source }
    }

    pub fn id(&self) -> u32 {
        self.child.id()
    }

    pub fn source(&self) -> Aria2BinarySource {
        self.source.clone()
    }

    pub(super) fn is_running(&mut self) -> Result<bool, String> {
        self.child
            .try_wait()
            .map(|status| status.is_none())
            .map_err(|error| format!("读取 Aria2 进程状态失败：{}", error))
    }

    pub fn kill(&mut self) -> Result<(), String> {
        self.child
            .kill()
            .map_err(|error| format!("停止 Aria2 进程句柄失败：{}", error))
    }

    pub(super) fn interrupt_and_wait(&mut self, timeout: Duration) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        // 持有未回收的 Child 身份，SIGINT 让引擎自行排空 I/O 和提交原生状态。
        if unsafe { libc::kill(self.child.id() as libc::pid_t, libc::SIGINT) } != 0 {
            return Err(format!(
                "请求 Aria2 正常退出失败：{}",
                std::io::Error::last_os_error()
            ));
        }
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) if status.success() => return Ok(()),
                Ok(Some(status)) => {
                    return Err(format!("Aria2 异常退出（{status}），无法确认原生状态提交"))
                }
                Err(error) => return Err(format!("等待 Aria2 正常退出失败：{error}")),
                Ok(None) => {}
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("Aria2 正常退出超时，无法确认原生状态提交，已保留运行态".to_string());
            }
            std::thread::sleep(remaining.min(Duration::from_millis(10)));
        }
    }

    pub(super) fn wait_for_exit(&mut self, timeout: Duration) -> Result<bool, String> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            match self.child.try_wait() {
                Ok(Some(_)) => return Ok(true),
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(error) => return Err(format!("等待 Aria2 进程退出失败：{}", error)),
            }
        }
        self.child
            .try_wait()
            .map(|status| status.is_some())
            .map_err(|error| format!("等待 Aria2 进程退出失败：{}", error))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAria2Binary {
    pub path: PathBuf,
    pub source: Aria2BinarySource,
}
