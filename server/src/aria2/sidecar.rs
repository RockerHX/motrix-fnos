use super::process_probe::{read_process_command_line, terminate_process};
use super::runtime_file::SavedAria2Runtime;
use crate::config::aria2::Aria2BinarySource;
use crate::debug_logs::DebugLogStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidecarOwnership {
    OwnSidecar,
    ExternalOrUnknown,
}

pub fn classify_saved_sidecar(
    saved: Option<&SavedAria2Runtime>,
    candidate_port: u16,
    debug_logs: &DebugLogStore,
) -> SidecarOwnership {
    let Some(runtime) = saved else {
        return SidecarOwnership::ExternalOrUnknown;
    };

    let command_line = match read_process_command_line(runtime.pid) {
        Ok(command_line) => command_line,
        Err(error) => {
            debug_logs.warn(
                "aria2.cleanup",
                format!("残留 sidecar 命令行读取失败，按未知进程处理：{}", error),
            );
            return SidecarOwnership::ExternalOrUnknown;
        }
    };

    classify_saved_sidecar_from_command_line(Some(runtime), candidate_port, Some(&command_line))
}

pub(super) fn classify_saved_sidecar_from_command_line(
    saved: Option<&SavedAria2Runtime>,
    candidate_port: u16,
    command_line: Option<&str>,
) -> SidecarOwnership {
    let Some(runtime) = saved else {
        return SidecarOwnership::ExternalOrUnknown;
    };

    if runtime.binary_source != Aria2BinarySource::Sidecar
        || runtime.actual_port != candidate_port
        || runtime.rpc_secret.trim().is_empty()
        || runtime.pid == 0
    {
        return SidecarOwnership::ExternalOrUnknown;
    }

    let Some(command_line) = command_line else {
        return SidecarOwnership::ExternalOrUnknown;
    };
    // PID 本身不能证明进程归属；只有 sidecar 名称、RPC 端口和 secret 等运行态证据同时匹配时才允许清理。
    let evidence = analyze_sidecar_command_line(command_line, runtime, candidate_port);

    if evidence.contains_sidecar_name
        && evidence.contains_rpc_port
        && evidence.contains_rpc_secret
        && evidence.matched_count() >= 3
    {
        SidecarOwnership::OwnSidecar
    } else {
        SidecarOwnership::ExternalOrUnknown
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SidecarCommandLineEvidence {
    pub contains_sidecar_name: bool,
    pub contains_rpc_port: bool,
    pub contains_rpc_secret: bool,
    pub contains_app_data_path: bool,
    pub contains_session_path: bool,
    pub contains_log_path: bool,
}

impl SidecarCommandLineEvidence {
    pub fn matched_count(&self) -> usize {
        [
            self.contains_sidecar_name,
            self.contains_rpc_port,
            self.contains_rpc_secret,
            self.contains_app_data_path,
            self.contains_session_path,
            self.contains_log_path,
        ]
        .into_iter()
        .filter(|matched| *matched)
        .count()
    }
}

pub(crate) fn analyze_sidecar_command_line(
    command_line: &str,
    runtime: &SavedAria2Runtime,
    candidate_port: u16,
) -> SidecarCommandLineEvidence {
    let normalized_command = normalize_path_text(command_line);

    SidecarCommandLineEvidence {
        contains_sidecar_name: runtime
            .sidecar_name
            .as_deref()
            .map(|name| !name.trim().is_empty() && command_line.contains(name))
            .unwrap_or(false),
        contains_rpc_port: command_line_contains_rpc_port(command_line, candidate_port),
        contains_rpc_secret: !runtime.rpc_secret.trim().is_empty()
            && command_line.contains(&format!("--rpc-secret={}", runtime.rpc_secret)),
        contains_app_data_path: optional_path_matches(
            &normalized_command,
            runtime.app_data_dir.as_deref(),
        ),
        contains_session_path: optional_path_matches(
            &normalized_command,
            runtime.aria2_session_path.as_deref(),
        ),
        contains_log_path: optional_path_matches(
            &normalized_command,
            runtime.aria2_log_path.as_deref(),
        ),
    }
}

fn command_line_contains_rpc_port(command_line: &str, candidate_port: u16) -> bool {
    let plain = format!("--rpc-listen-port={candidate_port}");
    let quoted = format!("--rpc-listen-port=\"{candidate_port}\"");
    command_line.contains(&plain) || command_line.contains(&quoted)
}

fn optional_path_matches(normalized_command: &str, path: Option<&str>) -> bool {
    path.map(normalize_path_text)
        .filter(|path| !path.trim().is_empty())
        .map(|path| normalized_command.contains(&path))
        .unwrap_or(false)
}

fn normalize_path_text(value: &str) -> String {
    value.replace('\\', "/")
}

pub fn cleanup_saved_sidecar_if_owned(
    saved: Option<&SavedAria2Runtime>,
    candidate_port: u16,
    debug_logs: &DebugLogStore,
) -> bool {
    let Some(runtime) = saved else {
        return false;
    };
    if runtime.actual_port != candidate_port {
        debug_logs.warn(
            "aria2.cleanup",
            format!(
                "跳过残留 sidecar 清理：运行态端口 {} 与候选端口 {} 不一致",
                runtime.actual_port, candidate_port
            ),
        );
        return false;
    }

    if !terminate_process(runtime.pid) {
        debug_logs.warn(
            "aria2.cleanup",
            format!("本应用残留 sidecar PID {} 清理未确认成功", runtime.pid),
        );
        return false;
    }

    debug_logs.info(
        "aria2.cleanup",
        format!(
            "已清理本应用残留 Aria2 sidecar，PID {}，端口 {}",
            runtime.pid, runtime.actual_port
        ),
    );
    true
}
