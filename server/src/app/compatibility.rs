use crate::config::aria2::EngineGeneration;
use crate::database::DATABASE_FILE_NAME;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::Row;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppCompatibility {
    pub engine_generation: EngineGeneration,
    pub mode: String,
    pub reason_code: Option<String>,
    pub unfinished_task_count: usize,
    pub unresolved_operation_count: usize,
    pub message: String,
}

impl AppCompatibility {
    pub fn normal(generation: EngineGeneration) -> Self {
        Self {
            engine_generation: generation,
            mode: "normal".to_string(),
            reason_code: None,
            unfinished_task_count: 0,
            unresolved_operation_count: 0,
            message: String::new(),
        }
    }

    pub(crate) fn restricted(generation: EngineGeneration, reason: &str) -> Self {
        Self {
            mode: "restricted".to_string(),
            reason_code: Some(reason.to_string()),
            message: "检测到不兼容或未完成迁移的旧版状态，下载功能暂不可用。请恢复升级前备份并安装原版本，处理旧任务后再升级。".to_string(),
            ..Self::normal(generation)
        }
    }

    pub fn ensure_download_available(&self) -> Result<(), String> {
        if self.mode == "normal" {
            Ok(())
        } else {
            Err(format!("engine_upgrade_required：{}", self.message))
        }
    }
}

#[derive(Default)]
pub(crate) struct LegacyInspection {
    pub(crate) tasks: BTreeMap<String, (String, Option<String>)>,
    pub(crate) has_legacy_state: bool,
}

pub(crate) async fn inspect_legacy_state(
    root: &Path,
) -> Result<LegacyInspection, AppCompatibility> {
    let invalid =
        || AppCompatibility::restricted(EngineGeneration::NativeV2, "legacy_state_invalid");
    let mut inspection = LegacyInspection::default();
    let database_path = root.join(DATABASE_FILE_NAME);
    if regular_file_exists(&database_path).map_err(|_| invalid())? {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&database_path)
                    .read_only(true),
            )
            .await
            .map_err(|_| invalid())?;
        let result = inspect_database(&pool, &mut inspection).await;
        pool.close().await;
        result?;
    }
    for relative in [
        "aria2/aria2.session",
        "magnet-metadata",
        "task-metadata",
        "aria2/state",
    ] {
        let path = root.join(relative);
        if relative.ends_with(".session") {
            let Some(content) = read_private_text(&path).map_err(|_| invalid())? else {
                continue;
            };
            if !content.trim().is_empty() {
                inspection.has_legacy_state = true;
                validate_legacy_session(&content, &inspection).map_err(|_| invalid())?;
            }
        } else if directory_has_entries(&path).map_err(|_| invalid())? {
            inspection.has_legacy_state = true;
        }
    }
    Ok(inspection)
}

async fn inspect_database(
    pool: &sqlx::SqlitePool,
    inspection: &mut LegacyInspection,
) -> Result<(), AppCompatibility> {
    let invalid =
        || AppCompatibility::restricted(EngineGeneration::NativeV2, "legacy_state_invalid");
    let integrity: String = sqlx::query_scalar("PRAGMA quick_check")
        .fetch_one(pool)
        .await
        .map_err(|_| invalid())?;
    if integrity != "ok" {
        return Err(invalid());
    }
    let migrations: Vec<(i64, String)> =
        sqlx::query_as("SELECT version, name FROM schema_migrations")
            .fetch_all(pool)
            .await
            .map_err(|_| invalid())?;
    if migrations.iter().any(|(version, _)| *version > 8) {
        return Err(invalid());
    }
    let trusted_legacy = migrations
        .iter()
        .any(|(version, name)| *version == 7 && name == "web_auth_bootstrap_token");
    let columns: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('download_tasks')")
            .fetch_all(pool)
            .await
            .map_err(|_| invalid())?;
    let has_generation = columns.iter().any(|column| column == "engine_generation");
    if !has_generation && !trusted_legacy {
        return Err(invalid());
    }
    let rows = sqlx::query(if has_generation {
        "SELECT id, status, gid, save_dir, engine_generation FROM download_tasks"
    } else {
        "SELECT id, status, gid, save_dir, 'legacy-v1' AS engine_generation FROM download_tasks"
    })
    .fetch_all(pool)
    .await
    .map_err(|_| invalid())?;
    let mut legacy_ids = BTreeSet::new();
    let mut directories = BTreeSet::new();
    let mut unfinished = 0;
    for row in rows {
        let generation: String = row.try_get("engine_generation").map_err(|_| invalid())?;
        if generation == "native-v2" {
            continue;
        }
        if generation != "legacy-v1" {
            return Err(invalid());
        }
        let id: i64 = row.try_get("id").map_err(|_| invalid())?;
        legacy_ids.insert(id);
        let status: String = row.try_get("status").map_err(|_| invalid())?;
        if !matches!(status.as_str(), "complete" | "removed") {
            unfinished += 1;
        }
        let gid: Option<String> = row.try_get("gid").map_err(|_| invalid())?;
        if let Some(gid) = gid.filter(|gid| !gid.is_empty()) {
            if inspection
                .tasks
                .insert(gid, (status, Some(id.to_string())))
                .is_some()
            {
                return Err(invalid());
            }
        }
        directories.insert(
            row.try_get::<String, _>("save_dir")
                .map_err(|_| invalid())?,
        );
    }
    inspection.has_legacy_state = !legacy_ids.is_empty();
    if unfinished > 0 {
        let mut status =
            AppCompatibility::restricted(EngineGeneration::NativeV2, "legacy_tasks_unfinished");
        status.unfinished_task_count = unfinished;
        return Err(status);
    }
    let operations =
        sqlx::query("SELECT task_id, status, phase, context_json FROM task_operations")
            .fetch_all(pool)
            .await
            .map_err(|_| invalid())?;
    let mut unresolved = 0;
    for operation in operations {
        let task_id: i64 = operation.try_get("task_id").map_err(|_| invalid())?;
        let context: serde_json::Value = serde_json::from_str(
            operation
                .try_get::<&str, _>("context_json")
                .map_err(|_| invalid())?,
        )
        .map_err(|_| invalid())?;
        let status: String = operation.try_get("status").map_err(|_| invalid())?;
        let phase: String = operation.try_get("phase").map_err(|_| invalid())?;
        if (legacy_ids.contains(&task_id)
            || context["taskSnapshot"]["engineGeneration"] != "native-v2")
            && (!matches!(status.as_str(), "completed" | "failed")
                || (status != "completed"
                    && matches!(
                        phase.as_str(),
                        "file_cleanup_pending"
                            | "file_staging_in_progress"
                            | "aria2_outcome_unknown"
                    )))
        {
            unresolved += 1;
        }
    }
    if unresolved > 0 {
        let mut status = AppCompatibility::restricted(
            EngineGeneration::NativeV2,
            "legacy_operations_unresolved",
        );
        status.unresolved_operation_count = unresolved;
        return Err(status);
    }
    for directory in directories {
        let path = Path::new(&directory);
        if directory_has_entries(path).map_err(|_| invalid())? {
            for entry in std::fs::read_dir(path).map_err(|_| invalid())? {
                let entry = entry.map_err(|_| invalid())?;
                if entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".motrix-redownload-backup-")
                {
                    return Err(AppCompatibility::restricted(
                        EngineGeneration::NativeV2,
                        "legacy_operations_unresolved",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_legacy_session(content: &str, inspection: &LegacyInspection) -> Result<(), String> {
    let mut has_entry = false;
    let mut gid = None;
    let mut seen = BTreeSet::new();
    let check = |gid: &Option<String>| -> Result<(), String> {
        if !gid
            .as_ref()
            .is_some_and(|gid| inspection.tasks.contains_key(gid))
        {
            return Err("session 包含无法关联的旧任务".to_string());
        }
        Ok(())
    };
    for line in content
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
    {
        if !line.starts_with(char::is_whitespace) {
            if has_entry {
                check(&gid)?;
            }
            has_entry = true;
            gid = None;
        } else if let Some(value) = line.trim().strip_prefix("gid=") {
            if gid.is_some()
                || value.len() != 16
                || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
                || !seen.insert(value.to_string())
            {
                return Err("session 身份无效".to_string());
            }
            gid = Some(value.to_string());
        } else if !has_entry || !line.contains('=') {
            return Err("session 内容无效".to_string());
        }
    }
    if has_entry {
        check(&gid)?;
    }
    Ok(())
}

pub(crate) fn regular_file_exists(path: &Path) -> Result<bool, String> {
    use std::os::unix::fs::MetadataExt;
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && metadata.nlink() == 1 => Ok(true),
        Ok(_) => Err("私有状态不是普通文件".to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err("无法读取私有状态".to_string()),
    }
}

pub(crate) fn read_private_text(path: &Path) -> Result<Option<String>, String> {
    if !regular_file_exists(path)? {
        return Ok(None);
    }
    std::fs::read_to_string(path)
        .map(Some)
        .map_err(|_| "无法读取私有状态".to_string())
}

pub(crate) fn directory_has_entries(path: &Path) -> Result<bool, String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            match std::fs::read_dir(path)
                .map_err(|_| "无法读取状态目录")?
                .next()
            {
                Some(Ok(_)) => Ok(true),
                Some(Err(_)) => Err("无法读取状态目录".to_string()),
                None => Ok(false),
            }
        }
        Ok(_) => Err("状态目录类型无效".to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err("无法读取状态目录".to_string()),
    }
}
