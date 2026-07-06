use crate::config::aria2::Aria2BinarySource;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Child;

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

    pub(super) fn wait(&mut self) {
        let _ = self.child.wait();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAria2Binary {
    pub path: PathBuf,
    pub source: Aria2BinarySource,
}
