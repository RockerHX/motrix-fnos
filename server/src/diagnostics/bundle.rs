use crate::app::HttpAppState;
use crate::debug_logs::{redact_log_message, DebugLogEntry};
use crate::runtime::process_status;
use serde::Serialize;
use std::fs::{self, File, OpenOptions};
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path};
use std::time::{SystemTime, UNIX_EPOCH};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

pub(crate) const DIAGNOSTIC_BUNDLE_MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const ARIA2_LOG_BUNDLE_MAX_BYTES: usize = 10 * 1024 * 1024;
pub(crate) const SERVER_LOG_BUNDLE_MAX_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const LIFECYCLE_LOG_BUNDLE_MAX_BYTES: usize = 2 * 1024 * 1024;
const APP_DEBUG_LOG_BUNDLE_MAX_BYTES: usize = 512 * 1024;
const SUMMARY_BUNDLE_MAX_BYTES: usize = 64 * 1024;
const HISTORY_LOG_MIN_TAIL_BYTES: usize = 256 * 1024;
const LOGIN_DIAGNOSTIC_BUNDLE_MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const LOGIN_DEBUG_LOG_BUNDLE_MAX_BYTES: usize = 256 * 1024;
const LOGIN_SUMMARY_BUNDLE_MAX_BYTES: usize = 16 * 1024;
const LOGIN_LIFECYCLE_LOG_BUNDLE_MAX_BYTES: usize = 512 * 1024;

const SERVER_LOG_FILES: &[(&str, &str)] = &[
    ("logs/server.log", "logs/server.log"),
    ("logs/server.log.1", "logs/server.log.1"),
    ("logs/server.log.2", "logs/server.log.2"),
    ("logs/server.log.3", "logs/server.log.3"),
];
const LIFECYCLE_LOG_FILES: &[(&str, &str)] = &[
    ("logs/lifecycle.log", "logs/lifecycle.log"),
    ("logs/lifecycle.log.1", "logs/lifecycle.log.1"),
    ("logs/lifecycle.log.2", "logs/lifecycle.log.2"),
    ("logs/lifecycle.log.3", "logs/lifecycle.log.3"),
];
const ARIA2_LOG_FILES: &[(&str, &str)] = &[
    ("aria2/aria2.log", "logs/aria2/aria2.log"),
    ("aria2/aria2.1.log", "logs/aria2/aria2.1.log"),
    ("aria2/aria2.2.log", "logs/aria2/aria2.2.log"),
    ("aria2/aria2.3.log", "logs/aria2/aria2.3.log"),
    ("aria2/aria2.log.1", "logs/aria2/aria2.log.1"),
    ("aria2/aria2.log.2", "logs/aria2/aria2.log.2"),
    ("aria2/aria2.log.3", "logs/aria2/aria2.log.3"),
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticSummary {
    generated_at_ms: u64,
    app_version: &'static str,
    aria2_version: Option<String>,
    aria2_running: bool,
    aria2_lifecycle_phase: String,
    aria2_log_mode: crate::aria2::Aria2LogModeStatus,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginDiagnosticSummary {
    generated_at_ms: u64,
    app_version: &'static str,
    management_listener: String,
    secure_cookie_enabled: bool,
    included_logs: &'static str,
}

struct ArchiveEntry {
    name: String,
    contents: String,
}

struct BundleBudget {
    remaining: usize,
}

struct SanitizedTail {
    contents: String,
    bytes_read: usize,
}

struct AvailableLogFile<'a> {
    relative_path: &'a str,
    archive_path: &'a str,
    bytes: usize,
}

pub(crate) fn build_diagnostic_bundle(state: &HttpAppState) -> Result<Vec<u8>, String> {
    let process = process_status(&state.aria2_process)?;
    let lifecycle = state.aria2_lifecycle.snapshot()?;
    let summary = DiagnosticSummary {
        generated_at_ms: current_timestamp_ms(),
        app_version: env!("CARGO_PKG_VERSION"),
        aria2_version: state.last_aria2_version(),
        aria2_running: process.running,
        aria2_lifecycle_phase: format!("{:?}", lifecycle.phase),
        aria2_log_mode: state.aria2_log_mode.status(process.running),
    };

    build_diagnostic_bundle_from_parts(
        &state.runtime.app_data_dir,
        &state.core.debug_logs.list(),
        summary,
    )
}

pub(crate) fn build_login_diagnostic_bundle(state: &HttpAppState) -> Result<Vec<u8>, String> {
    let summary = LoginDiagnosticSummary {
        generated_at_ms: current_timestamp_ms(),
        app_version: env!("CARGO_PKG_VERSION"),
        management_listener: state.runtime.http_addr.to_string(),
        secure_cookie_enabled: state.runtime.web_cookie_secure,
        included_logs: "脱敏鉴权记录与生命周期日志",
    };
    let debug_logs = state
        .core
        .debug_logs
        .list()
        .into_iter()
        .filter(|entry| entry.module == "auth.failure" || entry.module.starts_with("auth."))
        .collect::<Vec<_>>();
    build_login_diagnostic_bundle_from_parts(&state.runtime.app_data_dir, &debug_logs, summary)
}

fn build_login_diagnostic_bundle_from_parts(
    app_data_dir: &Path,
    debug_logs: &[DebugLogEntry],
    summary: LoginDiagnosticSummary,
) -> Result<Vec<u8>, String> {
    let mut entries = Vec::new();
    let mut budget = BundleBudget {
        remaining: LOGIN_DIAGNOSTIC_BUNDLE_MAX_INPUT_BYTES,
    };
    let summary = serde_json::to_string_pretty(&summary)
        .map_err(|error| format!("序列化登录诊断摘要失败：{error}"))?;
    append_text_entry(
        &mut entries,
        "summary.json",
        &summary,
        LOGIN_SUMMARY_BUNDLE_MAX_BYTES,
        &mut budget,
    );
    append_text_entry(
        &mut entries,
        "logs/auth-debug.jsonl",
        &format_debug_logs(
            &debug_logs
                .iter()
                .filter(|entry| entry.module == "auth.failure" || entry.module.starts_with("auth."))
                .cloned()
                .collect::<Vec<_>>(),
        ),
        LOGIN_DEBUG_LOG_BUNDLE_MAX_BYTES,
        &mut budget,
    );
    append_log_group(
        &mut entries,
        app_data_dir,
        LIFECYCLE_LOG_FILES,
        LOGIN_LIFECYCLE_LOG_BUNDLE_MAX_BYTES,
        &mut budget,
    );
    write_zip(entries)
}

fn build_diagnostic_bundle_from_parts(
    app_data_dir: &Path,
    debug_logs: &[DebugLogEntry],
    summary: DiagnosticSummary,
) -> Result<Vec<u8>, String> {
    let mut entries = Vec::new();
    let mut budget = BundleBudget {
        remaining: DIAGNOSTIC_BUNDLE_MAX_INPUT_BYTES,
    };

    let summary = serde_json::to_string_pretty(&summary)
        .map_err(|error| format!("序列化诊断摘要失败：{error}"))?;
    append_text_entry(
        &mut entries,
        "summary.json",
        &summary,
        SUMMARY_BUNDLE_MAX_BYTES,
        &mut budget,
    );
    append_text_entry(
        &mut entries,
        "logs/app-debug.jsonl",
        &format_debug_logs(debug_logs),
        APP_DEBUG_LOG_BUNDLE_MAX_BYTES,
        &mut budget,
    );
    append_log_group(
        &mut entries,
        app_data_dir,
        ARIA2_LOG_FILES,
        ARIA2_LOG_BUNDLE_MAX_BYTES,
        &mut budget,
    );
    append_log_group(
        &mut entries,
        app_data_dir,
        SERVER_LOG_FILES,
        SERVER_LOG_BUNDLE_MAX_BYTES,
        &mut budget,
    );
    append_log_group(
        &mut entries,
        app_data_dir,
        LIFECYCLE_LOG_FILES,
        LIFECYCLE_LOG_BUNDLE_MAX_BYTES,
        &mut budget,
    );

    write_zip(entries)
}

fn append_text_entry(
    entries: &mut Vec<ArchiveEntry>,
    name: &str,
    contents: &str,
    max_bytes: usize,
    budget: &mut BundleBudget,
) {
    let limit = max_bytes.min(budget.remaining);
    let contents = tail_text(&redact_lines(contents), limit);
    budget.remaining = budget.remaining.saturating_sub(contents.len());
    entries.push(ArchiveEntry {
        name: name.to_string(),
        contents,
    });
}

fn append_log_group(
    entries: &mut Vec<ArchiveEntry>,
    app_data_dir: &Path,
    files: &[(&str, &str)],
    max_bytes: usize,
    budget: &mut BundleBudget,
) {
    let mut group_remaining = max_bytes.min(budget.remaining);
    let available = files
        .iter()
        .filter_map(|(relative_path, archive_path)| {
            let file = open_regular_file(app_data_dir, relative_path)?;
            let bytes = usize::try_from(file.metadata().ok()?.len()).ok()?;
            Some(AvailableLogFile {
                relative_path,
                archive_path,
                bytes,
            })
        })
        .collect::<Vec<_>>();

    for (index, file) in available.iter().enumerate() {
        if group_remaining == 0 || budget.remaining == 0 {
            break;
        }
        let reserved_for_later = available[index + 1..]
            .iter()
            .map(|later| later.bytes.min(HISTORY_LOG_MIN_TAIL_BYTES))
            .sum::<usize>()
            .min(group_remaining);
        let file_limit = group_remaining
            .saturating_sub(reserved_for_later)
            .min(file.bytes);
        let Some(tail) = read_sanitized_tail(app_data_dir, file.relative_path, file_limit) else {
            continue;
        };
        group_remaining = group_remaining.saturating_sub(tail.bytes_read);
        budget.remaining = budget.remaining.saturating_sub(tail.bytes_read);
        entries.push(ArchiveEntry {
            name: file.archive_path.to_string(),
            contents: tail.contents,
        });
    }
}

fn read_sanitized_tail(
    app_data_dir: &Path,
    relative_path: &str,
    max_bytes: usize,
) -> Option<SanitizedTail> {
    if max_bytes == 0 {
        return None;
    }
    let mut file = open_regular_file(app_data_dir, relative_path)?;
    let file_size = file.metadata().ok()?.len();
    let bytes_read = usize::try_from(file_size.min(max_bytes as u64)).ok()?;
    let offset = file_size.saturating_sub(bytes_read as u64);
    file.seek(SeekFrom::Start(offset)).ok()?;

    let mut bytes = vec![0; bytes_read];
    file.read_exact(&mut bytes).ok()?;
    let bytes = if offset > 0 {
        match bytes.iter().position(|byte| *byte == b'\n') {
            Some(index) if index + 1 < bytes.len() => bytes.split_off(index + 1),
            None => bytes,
            Some(_) => bytes,
        }
    } else {
        bytes
    };
    let contents = String::from_utf8_lossy(&bytes).into_owned();

    Some(SanitizedTail {
        contents: tail_text(&redact_lines(&contents), max_bytes),
        bytes_read,
    })
}

fn open_regular_file(app_data_dir: &Path, relative_path: &str) -> Option<File> {
    let mut path = app_data_dir.to_path_buf();
    for component in Path::new(relative_path).components() {
        let Component::Normal(component) = component else {
            return None;
        };
        path.push(component);
        if fs::symlink_metadata(&path).ok()?.file_type().is_symlink() {
            return None;
        }
    }

    let metadata = fs::symlink_metadata(&path).ok()?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return None;
    }

    #[cfg(unix)]
    let file = {
        use std::os::unix::fs::OpenOptionsExt;
        OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .ok()?
    };
    #[cfg(not(unix))]
    let file = OpenOptions::new().read(true).open(path).ok()?;

    file.metadata().ok()?.is_file().then_some(file)
}

fn format_debug_logs(debug_logs: &[DebugLogEntry]) -> String {
    debug_logs
        .iter()
        .map(|entry| {
            serde_json::to_string(entry)
                .map(|line| redact_log_message(&line))
                .unwrap_or_else(|_| "{\"message\":\"[REDACTED]\"}".to_string())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn redact_lines(contents: &str) -> String {
    contents
        .split('\n')
        .map(redact_log_message)
        .collect::<Vec<_>>()
        .join("\n")
}

fn tail_text(contents: &str, max_bytes: usize) -> String {
    if contents.len() <= max_bytes {
        return contents.to_string();
    }
    let mut start = contents.len().saturating_sub(max_bytes);
    while start < contents.len() && !contents.is_char_boundary(start) {
        start += 1;
    }
    let tail = &contents[start..];
    tail.find('\n')
        .map(|newline| tail[newline + 1..].to_string())
        .unwrap_or_else(|| tail.to_string())
}

fn write_zip(entries: Vec<ArchiveEntry>) -> Result<Vec<u8>, String> {
    let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for entry in entries {
        archive
            .start_file(&entry.name, options)
            .map_err(|error| format!("写入诊断包条目失败：{error}"))?;
        archive
            .write_all(entry.contents.as_bytes())
            .map_err(|error| format!("写入诊断包内容失败：{error}"))?;
    }
    archive
        .finish()
        .map(|cursor| cursor.into_inner())
        .map_err(|error| format!("完成诊断包失败：{error}"))
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
