use crate::app::HttpAppState;
use crate::aria2::{ARIA2_LOG_MAX_BYTES, ARIA2_LOG_MAX_FILES};
use crate::debug_logs::DEFAULT_FILE_LOG_RETENTION;
use crate::state::{ARIA2_LOG_FILE_NAME, ARIA2_RUNTIME_DIR_NAME};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{process_status, Aria2LifecyclePhase};

const ARIA2_LOG_HISTORY_RETENTION: usize = ARIA2_LOG_MAX_FILES - 1;
const APPLICATION_LOG_DIRECTORY_NAME: &str = "logs";
const SERVER_LOG_FILE_NAME: &str = "server.log";
const LIFECYCLE_LOG_FILE_NAME: &str = "lifecycle.log";
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Aria2LogMaintenanceReport {
    pub truncated_bytes: u64,
    pub removed_current_bytes: u64,
    pub removed_history_bytes: u64,
    pub removed_history_files: usize,
}

impl Aria2LogMaintenanceReport {
    pub(crate) fn reclaimed_bytes(self) -> u64 {
        self.truncated_bytes
            .saturating_add(self.removed_current_bytes)
            .saturating_add(self.removed_history_bytes)
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LogFileUsage {
    pub current_bytes: u64,
    pub history_bytes: u64,
    pub total_bytes: u64,
    pub current_file_count: usize,
    pub history_file_count: usize,
    pub total_file_count: usize,
}

impl LogFileUsage {
    fn add_current(&mut self, bytes: u64) {
        self.current_bytes = self.current_bytes.saturating_add(bytes);
        self.current_file_count = self.current_file_count.saturating_add(1);
        self.refresh_totals();
    }

    fn add_history(&mut self, bytes: u64) {
        self.history_bytes = self.history_bytes.saturating_add(bytes);
        self.history_file_count = self.history_file_count.saturating_add(1);
        self.refresh_totals();
    }

    fn refresh_totals(&mut self) {
        self.total_bytes = self.current_bytes.saturating_add(self.history_bytes);
        self.total_file_count = self
            .current_file_count
            .saturating_add(self.history_file_count);
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LogUsageSnapshot {
    pub aria2: LogFileUsage,
    pub server: LogFileUsage,
    pub lifecycle: LogFileUsage,
    pub total_bytes: u64,
    pub total_file_count: usize,
}

impl LogUsageSnapshot {
    fn refresh_totals(&mut self) {
        self.total_bytes = self
            .aria2
            .total_bytes
            .saturating_add(self.server.total_bytes)
            .saturating_add(self.lifecycle.total_bytes);
        self.total_file_count = self
            .aria2
            .total_file_count
            .saturating_add(self.server.total_file_count)
            .saturating_add(self.lifecycle.total_file_count);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Aria2LogMaintenanceSkipReason {
    ProcessRunning,
    LifecycleNotStopped(Aria2LifecyclePhase),
    RuntimeInMemory,
    RuntimeRecordPresent,
}

impl std::fmt::Display for Aria2LogMaintenanceSkipReason {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProcessRunning => formatter.write_str("Aria2 进程仍在运行"),
            Self::LifecycleNotStopped(phase) => {
                write!(formatter, "Aria2 当前处于 {:?} 生命周期阶段", phase)
            }
            Self::RuntimeInMemory => formatter.write_str("Aria2 内存运行态仍存在"),
            Self::RuntimeRecordPresent => formatter.write_str("Aria2 磁盘运行态记录仍存在"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Aria2LogMaintenanceOutcome {
    Maintained(Aria2LogMaintenanceReport),
    Skipped(Aria2LogMaintenanceSkipReason),
}

struct HistoryLog {
    path: PathBuf,
    name: String,
    bytes: u64,
    index: u64,
    spdlog: bool,
    modified_at: SystemTime,
}

pub(crate) async fn maintain_startup_aria2_logs(state: &HttpAppState) {
    match maintain_aria2_logs(state).await {
        Ok(Aria2LogMaintenanceOutcome::Maintained(report)) if report.reclaimed_bytes() > 0 => {
            state.core.debug_logs.info(
                "aria2.log_maintenance",
                format!(
                    "启动时已收敛 Aria2 日志，释放 {} 字节，清理历史文件 {} 个",
                    report.reclaimed_bytes(),
                    report.removed_history_files
                ),
            );
        }
        Ok(Aria2LogMaintenanceOutcome::Maintained(_)) => {}
        Ok(Aria2LogMaintenanceOutcome::Skipped(reason)) => {
            state.core.debug_logs.warn(
                "aria2.log_maintenance",
                format!("启动时跳过 Aria2 日志维护：{reason}"),
            );
        }
        Err(error) => {
            state.core.debug_logs.warn(
                "aria2.log_maintenance",
                format!("启动时无法安全维护 Aria2 日志：{error}"),
            );
        }
    }
}

pub(crate) async fn maintain_aria2_logs(
    state: &HttpAppState,
) -> Result<Aria2LogMaintenanceOutcome, String> {
    run_aria2_log_maintenance(state, maintain_aria2_log_files).await
}

pub(crate) async fn clear_aria2_logs(
    state: &HttpAppState,
) -> Result<Aria2LogMaintenanceOutcome, String> {
    run_aria2_log_maintenance(state, clear_aria2_log_files).await
}

async fn run_aria2_log_maintenance(
    state: &HttpAppState,
    operation: fn(&Path) -> Result<Aria2LogMaintenanceReport, String>,
) -> Result<Aria2LogMaintenanceOutcome, String> {
    let initial_lifecycle = state.aria2_lifecycle.snapshot()?;
    if initial_lifecycle.phase != Aria2LifecyclePhase::Stopped {
        return Ok(Aria2LogMaintenanceOutcome::Skipped(
            Aria2LogMaintenanceSkipReason::LifecycleNotStopped(initial_lifecycle.phase),
        ));
    }

    let _operation = state.aria2_lifecycle.lock_lifecycle_operation().await;
    let process = process_status(&state.aria2_process)?;
    if process.running {
        return Ok(Aria2LogMaintenanceOutcome::Skipped(
            Aria2LogMaintenanceSkipReason::ProcessRunning,
        ));
    }

    let lifecycle = state.aria2_lifecycle.snapshot()?;
    if lifecycle.phase != Aria2LifecyclePhase::Stopped {
        return Ok(Aria2LogMaintenanceOutcome::Skipped(
            Aria2LogMaintenanceSkipReason::LifecycleNotStopped(lifecycle.phase),
        ));
    }

    if state.aria2_runtime_snapshot().is_some() {
        return Ok(Aria2LogMaintenanceOutcome::Skipped(
            Aria2LogMaintenanceSkipReason::RuntimeInMemory,
        ));
    }

    if runtime_record_exists(&state.core.aria2_runtime_path)? {
        return Ok(Aria2LogMaintenanceOutcome::Skipped(
            Aria2LogMaintenanceSkipReason::RuntimeRecordPresent,
        ));
    }

    operation(&state.runtime.app_data_dir).map(Aria2LogMaintenanceOutcome::Maintained)
}

pub(crate) fn collect_log_usage(app_data_dir: &Path) -> Result<LogUsageSnapshot, String> {
    let mut usage = LogUsageSnapshot {
        aria2: collect_aria2_log_usage(app_data_dir)?,
        server: collect_fixed_log_usage(
            app_data_dir,
            SERVER_LOG_FILE_NAME,
            DEFAULT_FILE_LOG_RETENTION,
        )?,
        lifecycle: collect_fixed_log_usage(
            app_data_dir,
            LIFECYCLE_LOG_FILE_NAME,
            DEFAULT_FILE_LOG_RETENTION,
        )?,
        ..LogUsageSnapshot::default()
    };
    usage.refresh_totals();
    Ok(usage)
}

fn runtime_record_exists(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("读取 Aria2 运行态元数据失败：{error}")),
    }
}

fn maintain_aria2_log_files(app_data_dir: &Path) -> Result<Aria2LogMaintenanceReport, String> {
    let Some(log_dir) = aria2_log_directory(app_data_dir)? else {
        return Ok(Aria2LogMaintenanceReport::default());
    };
    let histories = collect_history_logs(&log_dir)?;
    let current_log = log_dir.join(ARIA2_LOG_FILE_NAME);
    let truncated_bytes = truncate_current_log(&current_log)?;
    let (removed_history_bytes, removed_history_files) = remove_excess_history_logs(histories)?;

    Ok(Aria2LogMaintenanceReport {
        truncated_bytes,
        removed_current_bytes: 0,
        removed_history_bytes,
        removed_history_files,
    })
}

fn clear_aria2_log_files(app_data_dir: &Path) -> Result<Aria2LogMaintenanceReport, String> {
    let Some(log_dir) = aria2_log_directory(app_data_dir)? else {
        return Ok(Aria2LogMaintenanceReport::default());
    };
    let histories = collect_history_logs(&log_dir)?;
    let removed_current_bytes = remove_current_log(&log_dir.join(ARIA2_LOG_FILE_NAME))?;
    let (removed_history_bytes, removed_history_files) = remove_history_logs(histories)?;

    Ok(Aria2LogMaintenanceReport {
        truncated_bytes: 0,
        removed_current_bytes,
        removed_history_bytes,
        removed_history_files,
    })
}

fn collect_aria2_log_usage(app_data_dir: &Path) -> Result<LogFileUsage, String> {
    let Some(log_dir) = aria2_log_directory_for_read(app_data_dir)? else {
        return Ok(LogFileUsage::default());
    };

    let mut usage = LogFileUsage::default();
    if let Some(bytes) = regular_file_size(&log_dir.join(ARIA2_LOG_FILE_NAME))? {
        usage.add_current(bytes);
    }
    for history in collect_history_logs(&log_dir)? {
        usage.add_history(history.bytes);
    }
    Ok(usage)
}

fn collect_fixed_log_usage(
    app_data_dir: &Path,
    file_name: &str,
    history_retention: usize,
) -> Result<LogFileUsage, String> {
    let log_dir = app_data_dir.join(APPLICATION_LOG_DIRECTORY_NAME);
    if !trusted_directory_for_read(&log_dir)? {
        return Ok(LogFileUsage::default());
    }

    let mut usage = LogFileUsage::default();
    if let Some(bytes) = regular_file_size(&log_dir.join(file_name))? {
        usage.add_current(bytes);
    }
    for index in 1..=history_retention {
        let path = log_dir.join(format!("{file_name}.{index}"));
        if let Some(bytes) = regular_file_size(&path)? {
            usage.add_history(bytes);
        }
    }
    Ok(usage)
}

fn aria2_log_directory(app_data_dir: &Path) -> Result<Option<PathBuf>, String> {
    let log_dir = app_data_dir.join(ARIA2_RUNTIME_DIR_NAME);
    match fs::symlink_metadata(&log_dir) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err("Aria2 日志目录不是受信任的普通目录".to_string())
        }
        Ok(_) => Ok(Some(log_dir)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("读取 Aria2 日志目录元数据失败：{error}")),
    }
}

fn aria2_log_directory_for_read(app_data_dir: &Path) -> Result<Option<PathBuf>, String> {
    let app_data_metadata = match fs::symlink_metadata(app_data_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("读取应用数据目录元数据失败：{error}")),
    };
    if app_data_metadata.file_type().is_symlink() || !app_data_metadata.is_dir() {
        return Ok(None);
    }

    let log_dir = app_data_dir.join(ARIA2_RUNTIME_DIR_NAME);
    match fs::symlink_metadata(&log_dir) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => Ok(None),
        Ok(_) => Ok(Some(log_dir)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("读取 Aria2 日志目录元数据失败：{error}")),
    }
}

fn trusted_directory_for_read(path: &Path) -> Result<bool, String> {
    let Some(parent) = path.parent() else {
        return Ok(false);
    };
    let parent_metadata = match fs::symlink_metadata(parent) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("读取应用数据日志目录元数据失败：{error}")),
    };
    if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
        return Ok(false);
    }

    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(!metadata.file_type().is_symlink() && metadata.is_dir()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("读取应用数据日志目录元数据失败：{error}")),
    }
}

fn regular_file_size(path: &Path) -> Result<Option<u64>, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => Ok(None),
        Ok(metadata) => Ok(Some(metadata.len())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("读取日志文件元数据失败：{error}")),
    }
}

fn collect_history_logs(log_dir: &Path) -> Result<Vec<HistoryLog>, String> {
    let mut histories = Vec::new();
    let entries =
        fs::read_dir(log_dir).map_err(|error| format!("读取 Aria2 日志目录失败：{error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("读取 Aria2 日志目录条目失败：{error}"))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((spdlog, index)) = parse_history_name(&name) else {
            continue;
        };

        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("读取 Aria2 历史日志元数据失败：{error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            continue;
        }
        histories.push(HistoryLog {
            path,
            name,
            bytes: metadata.len(),
            index,
            spdlog,
            modified_at: metadata.modified().unwrap_or(UNIX_EPOCH),
        });
    }
    histories.sort_by(|left, right| {
        let left_in_retention = left.index <= ARIA2_LOG_HISTORY_RETENTION as u64;
        let right_in_retention = right.index <= ARIA2_LOG_HISTORY_RETENTION as u64;
        right_in_retention
            .cmp(&left_in_retention)
            .then_with(|| right.spdlog.cmp(&left.spdlog))
            .then_with(|| left.index.cmp(&right.index))
            .then_with(|| right.modified_at.cmp(&left.modified_at))
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(histories)
}

fn parse_history_name(name: &str) -> Option<(bool, u64)> {
    if let Some(value) = name
        .strip_prefix("aria2.")
        .and_then(|value| value.strip_suffix(".log"))
    {
        return value
            .parse::<u64>()
            .ok()
            .filter(|index| *index > 0)
            .map(|index| (true, index));
    }

    name.strip_prefix("aria2.log.")
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|index| *index > 0)
        .map(|index| (false, index))
}

fn truncate_current_log(path: &Path) -> Result<u64, String> {
    if matches!(fs::symlink_metadata(path), Err(error) if error.kind() == io::ErrorKind::NotFound) {
        return Ok(0);
    }
    let mut source = open_regular_log_file(path)?;
    let metadata = source
        .metadata()
        .map_err(|error| format!("读取 Aria2 当前日志元数据失败：{error}"))?;
    let original_bytes = metadata.len();
    if original_bytes <= ARIA2_LOG_MAX_BYTES {
        return Ok(0);
    }

    source
        .seek(SeekFrom::Start(
            original_bytes.saturating_sub(ARIA2_LOG_MAX_BYTES),
        ))
        .map_err(|error| format!("定位 Aria2 当前日志尾部失败：{error}"))?;
    let parent = path
        .parent()
        .ok_or_else(|| "Aria2 当前日志缺少父目录".to_string())?;
    let (temporary_path, mut temporary) = create_temporary_log_file(parent)?;
    let result = (|| -> Result<(), String> {
        temporary
            .set_permissions(metadata.permissions())
            .map_err(|error| format!("设置 Aria2 临时日志权限失败：{error}"))?;
        let copied = {
            let mut tail = Read::by_ref(&mut source).take(ARIA2_LOG_MAX_BYTES);
            io::copy(&mut tail, &mut temporary)
                .map_err(|error| format!("写入 Aria2 当前日志尾部失败：{error}"))?
        };
        if copied != ARIA2_LOG_MAX_BYTES {
            return Err("读取 Aria2 当前日志尾部不完整，已跳过替换".to_string());
        }
        if source
            .metadata()
            .map_err(|error| format!("复核 Aria2 当前日志元数据失败：{error}"))?
            .len()
            != original_bytes
        {
            return Err("Aria2 当前日志在维护期间发生变化，已跳过替换".to_string());
        }
        temporary
            .sync_all()
            .map_err(|error| format!("同步 Aria2 临时日志失败：{error}"))?;
        drop(temporary);
        fs::rename(&temporary_path, path)
            .map_err(|error| format!("原子替换 Aria2 当前日志失败：{error}"))?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result?;
    Ok(original_bytes.saturating_sub(ARIA2_LOG_MAX_BYTES))
}

fn create_temporary_log_file(parent: &Path) -> Result<(PathBuf, File), String> {
    for _ in 0..16 {
        let sequence = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".{ARIA2_LOG_FILE_NAME}.motrix-maintenance-{}-{sequence}",
            std::process::id()
        ));
        match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("创建 Aria2 临时日志失败：{error}")),
        }
    }
    Err("无法创建唯一的 Aria2 临时日志文件".to_string())
}

fn open_regular_log_file(path: &Path) -> Result<File, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err("Aria2 当前日志不存在".to_string())
        }
        Err(error) => return Err(format!("读取 Aria2 当前日志元数据失败：{error}")),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("Aria2 当前日志不是受信任的普通文件".to_string());
    }

    #[cfg(unix)]
    let file = {
        use std::os::unix::fs::OpenOptionsExt;
        OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .map_err(|error| format!("打开 Aria2 当前日志失败：{error}"))?
    };
    #[cfg(not(unix))]
    let file = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|error| format!("打开 Aria2 当前日志失败：{error}"))?;

    if !file
        .metadata()
        .map_err(|error| format!("复核 Aria2 当前日志元数据失败：{error}"))?
        .is_file()
    {
        return Err("Aria2 当前日志不是普通文件".to_string());
    }
    Ok(file)
}

fn remove_excess_history_logs(histories: Vec<HistoryLog>) -> Result<(u64, usize), String> {
    remove_history_logs(
        histories
            .into_iter()
            .skip(ARIA2_LOG_HISTORY_RETENTION)
            .collect(),
    )
}

fn remove_history_logs(histories: Vec<HistoryLog>) -> Result<(u64, usize), String> {
    let mut removed_bytes = 0_u64;
    let mut removed_files = 0_usize;
    for history in histories {
        let metadata = fs::symlink_metadata(&history.path)
            .map_err(|error| format!("复核 Aria2 历史日志元数据失败：{error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            continue;
        }
        fs::remove_file(&history.path)
            .map_err(|error| format!("删除过期 Aria2 历史日志失败：{error}"))?;
        removed_bytes = removed_bytes.saturating_add(history.bytes);
        removed_files = removed_files.saturating_add(1);
    }
    Ok((removed_bytes, removed_files))
}

fn remove_current_log(path: &Path) -> Result<u64, String> {
    let Some(bytes) = regular_file_size(path)? else {
        if matches!(
            fs::symlink_metadata(path),
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file()
        ) {
            return Err("Aria2 当前日志不是受信任的普通文件".to_string());
        }
        return Ok(0);
    };
    fs::remove_file(path).map_err(|error| format!("删除 Aria2 当前日志失败：{error}"))?;
    Ok(bytes)
}

#[cfg(test)]
mod tests;
