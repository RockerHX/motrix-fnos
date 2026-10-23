use crate::database::settings::get_download_proxy_config;
use crate::tasks::{
    DownloadTask, DownloadTaskFile, DownloadTaskSourceType, DownloadTaskStatus, TaskProxyBinding,
    TaskProxySource,
};
use crate::{
    database::task_operations::update_task_operation_in_transaction, tasks::TaskOperation,
};
use sha2::{Digest, Sha256};
use sqlx::{Decode, Row, Sqlite, SqlitePool, Transaction, Type};
use std::path::Path;

pub async fn upsert_download_task(pool: &SqlitePool, task: &DownloadTask) -> Result<(), String> {
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| format!("启动保存下载任务事务失败：{}", error))?;
    upsert_download_task_in_transaction(&mut transaction, task).await?;
    transaction
        .commit()
        .await
        .map_err(|error| format!("提交保存下载任务事务失败：{}", error))?;
    Ok(())
}

async fn upsert_download_task_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    task: &DownloadTask,
) -> Result<(), String> {
    let result = sqlx::query(
        r#"
        INSERT INTO download_tasks (
            id, engine_generation, url, source_type, file_name, save_dir, owned_task_dir, category, gid, status, total_length, completed_length,
            download_speed, error_code, error_message, file_path, metadata_torrent_path, files_deleted,
            selected_file_indexes, confirmation_required, use_proxy, proxy_source, created_at, updated_at, removed_from_status
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            url = excluded.url,
            source_type = excluded.source_type,
            file_name = excluded.file_name,
            save_dir = excluded.save_dir,
            owned_task_dir = excluded.owned_task_dir,
            category = excluded.category,
            gid = excluded.gid,
            status = excluded.status,
            total_length = excluded.total_length,
            completed_length = excluded.completed_length,
            download_speed = excluded.download_speed,
            error_code = excluded.error_code,
            error_message = excluded.error_message,
            file_path = excluded.file_path,
            metadata_torrent_path = excluded.metadata_torrent_path,
            files_deleted = excluded.files_deleted,
            selected_file_indexes = excluded.selected_file_indexes,
            confirmation_required = excluded.confirmation_required,
            use_proxy = excluded.use_proxy,
            proxy_source = excluded.proxy_source,
            updated_at = excluded.updated_at,
            removed_from_status = excluded.removed_from_status
        WHERE download_tasks.engine_generation = excluded.engine_generation
        "#,
    )
    .bind(u64_to_i64(task.id, "任务 ID")?)
    .bind(task.engine_generation.as_storage_value())
    .bind(&task.url)
    .bind(task.source_type.as_storage_value())
    .bind(&task.file_name)
    .bind(&task.save_dir)
    .bind(&task.owned_task_dir)
    .bind(&task.category)
    .bind(&task.gid)
    .bind(task.status.as_storage_value())
    .bind(u64_to_i64(task.total_length, "总大小")?)
    .bind(u64_to_i64(if task.files_deleted { 0 } else { task.completed_length }, "已下载大小")?)
    .bind(u64_to_i64(task.download_speed, "下载速度")?)
    .bind(&task.error_code)
    .bind(&task.error_message)
    .bind(&task.file_path)
    .bind(&task.metadata_torrent_path)
    .bind(if task.files_deleted { 1_i64 } else { 0_i64 })
    .bind(serde_json::to_string(&task.selected_file_indexes).map_err(|error| {
        format!("序列化任务文件选择失败：{}", error)
    })?)
    .bind(if task.confirmation_required { 1_i64 } else { 0_i64 })
    .bind(if task.source_type == DownloadTaskSourceType::Url && task.use_proxy {
        1_i64
    } else {
        0_i64
    })
    .bind(if task.source_type == DownloadTaskSourceType::Url && task.use_proxy {
        task.proxy_binding.source()
    } else {
        TaskProxySource::Profile
    }
    .as_storage_value())
    .bind(u64_to_i64(task.created_at, "创建时间")?)
    .bind(u64_to_i64(task.updated_at, "更新时间")?)
    .bind(task.removed_from_status.as_ref().map(DownloadTaskStatus::as_storage_value))
    .execute(&mut **transaction)
    .await
    .map_err(|error| format!("保存下载任务失败：{}", error))?;
    if result.rows_affected() == 0 {
        return Err("任务引擎代际与持久化身份不一致，拒绝覆盖".to_string());
    }

    persist_task_proxy_override_in_transaction(transaction, task).await?;
    persist_task_files_in_transaction(transaction, task).await?;
    persist_task_metadata_ref_in_transaction(transaction, task).await?;

    Ok(())
}

async fn persist_task_files_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    task: &DownloadTask,
) -> Result<(), String> {
    let task_id = u64_to_i64(task.id, "任务 ID")?;
    sqlx::query("DELETE FROM task_file_snapshots WHERE task_id = ?")
        .bind(task_id)
        .execute(&mut **transaction)
        .await
        .map_err(|error| format!("更新任务文件快照失败：{error}"))?;
    for file in &task.files {
        let path = if task.engine_generation == crate::config::aria2::EngineGeneration::NativeV2 {
            let relative = Path::new(&file.path)
                .strip_prefix(crate::tasks::files::task_download_dir(task))
                .map_err(|_| "文件快照不属于任务目录".to_string())?;
            if relative.as_os_str().is_empty()
                || relative
                    .components()
                    .any(|part| !matches!(part, std::path::Component::Normal(_)))
            {
                return Err("文件快照路径不安全".to_string());
            }
            relative.to_string_lossy().into_owned()
        } else {
            file.path.clone()
        };
        sqlx::query("INSERT INTO task_file_snapshots (task_id, file_index, path, name, length, completed_length, selected, checked_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(task_id)
            .bind(i64::from(file.index))
            .bind(path)
            .bind(&file.name)
            .bind(u64_to_i64(file.length, "文件大小")?)
            .bind(u64_to_i64(if task.files_deleted { 0 } else { file.completed_length }, "文件有效进度")?)
            .bind(file.selected)
            .bind(u64_to_i64(task.updated_at, "文件核对时间")?)
            .execute(&mut **transaction)
            .await
            .map_err(|error| format!("保存任务文件快照失败：{error}"))?;
    }
    Ok(())
}

async fn persist_task_metadata_ref_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    task: &DownloadTask,
) -> Result<(), String> {
    let reference = if let Some(path) = task.metadata_torrent_path.as_deref() {
        Some(("torrent", path))
    } else if task.engine_generation == crate::config::aria2::EngineGeneration::NativeV2
        && task.source_type != DownloadTaskSourceType::Url
    {
        Some(("native-bt", "engine/native-v2/state"))
    } else {
        None
    };
    let Some((kind, path)) = reference else {
        return Ok(());
    };
    let (size, checksum, available) = if kind == "torrent" {
        let previous: Option<(Option<i64>, Option<String>)> = sqlx::query_as("SELECT size_bytes, checksum FROM task_metadata_refs WHERE task_id = ? AND private_path = ? AND kind = 'torrent'")
            .bind(u64_to_i64(task.id, "任务 ID")?).bind(path).fetch_optional(&mut **transaction).await
            .map_err(|error| format!("读取已有种子引用失败：{error}"))?;
        match std::fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
                if let Some((Some(size), Some(checksum))) = previous {
                    if size != u64_to_i64(metadata.len(), "种子大小")? {
                        return Err("私有种子内容已变化，保留原校验信息并拒绝覆盖".to_string());
                    }
                    (Some(size), Some(checksum), true)
                } else {
                    let bytes = std::fs::read(path)
                        .map_err(|error| format!("读取私有种子校验信息失败：{error}"))?;
                    (
                        Some(u64_to_i64(bytes.len() as u64, "种子大小")?),
                        Some(format!("{:x}", Sha256::digest(bytes))),
                        true,
                    )
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let (size, checksum) = previous.unwrap_or_default();
                (size, checksum, false)
            }
            Ok(_) => return Err("私有种子引用不是普通文件，已保留".to_string()),
            Err(error) => return Err(format!("读取私有种子引用失败：{error}")),
        }
    } else {
        // 原生引用由引擎管理；文件列表不能证明完整 metadata 已提交，290-06 再按原 GID 核实可用状态。
        (None, None, false)
    };
    sqlx::query("INSERT INTO task_metadata_refs (task_id, engine_generation, gid, kind, private_path, size_bytes, checksum, available, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(task_id) DO UPDATE SET gid = excluded.gid, kind = excluded.kind, private_path = excluded.private_path, size_bytes = excluded.size_bytes, checksum = excluded.checksum, available = excluded.available, updated_at = excluded.updated_at")
        .bind(u64_to_i64(task.id, "任务 ID")?)
        .bind(task.engine_generation.as_storage_value())
        .bind(&task.gid)
        .bind(kind)
        .bind(path)
        .bind(size)
        .bind(checksum)
        .bind(available)
        .bind(u64_to_i64(task.updated_at, "引用更新时间")?)
        .execute(&mut **transaction)
        .await
        .map_err(|error| format!("保存任务 metadata 引用失败：{error}"))?;
    Ok(())
}

pub(crate) async fn validate_native_metadata_refs(
    pool: &SqlitePool,
    root: &Path,
    tasks: &[DownloadTask],
) -> Result<(), String> {
    let rows = sqlx::query("SELECT task_id, engine_generation, gid, kind, private_path, size_bytes, checksum FROM task_metadata_refs WHERE NOT EXISTS (SELECT 1 FROM download_tasks WHERE id = task_id AND engine_generation = 'legacy-v1')")
        .fetch_all(pool).await.map_err(|error| format!("读取私有 metadata 引用失败：{error}"))?;
    let references = rows
        .into_iter()
        .map(|row| {
            Ok((
                i64_to_u64(get(&row, "task_id")?, "任务 ID")?,
                (
                    get::<String>(&row, "engine_generation")?,
                    get::<Option<String>>(&row, "gid")?,
                    get::<String>(&row, "kind")?,
                    get::<String>(&row, "private_path")?,
                    get::<Option<i64>>(&row, "size_bytes")?,
                    get::<Option<String>>(&row, "checksum")?,
                ),
            ))
        })
        .collect::<Result<std::collections::HashMap<_, _>, String>>()?;
    let root = root.to_path_buf();
    let tasks = tasks.to_vec();
    tokio::task::spawn_blocking(move || {
        let root = std::fs::canonicalize(root)
            .map_err(|error| format!("读取应用私有目录失败：{error}"))?;
        for task in tasks.iter().filter(|task| {
            task.engine_generation == crate::config::aria2::EngineGeneration::NativeV2
                && task.source_type != DownloadTaskSourceType::Url
        }) {
            let invalid = || {
                format!(
                    "task_resume_state_invalid：任务 {} 的私有 metadata 引用无法核对，已保留文件",
                    task.id
                )
            };
            let (generation, gid, kind, path, size, checksum) =
                references.get(&task.id).ok_or_else(invalid)?;
            if generation != "native-v2" || gid != &task.gid {
                return Err(invalid());
            }
            if kind == "native-bt"
                && path == "engine/native-v2/state"
                && task.metadata_torrent_path.is_none()
            {
                continue;
            }
            if kind != "torrent" || task.metadata_torrent_path.as_deref() != Some(path.as_str()) {
                return Err(invalid());
            }
            let expected = root.join(format!("task-metadata/task-{}/source.torrent", task.id));
            if std::fs::canonicalize(path).map_err(|_| invalid())? != expected {
                return Err(invalid());
            }
            let metadata = std::fs::symlink_metadata(&expected).map_err(|_| invalid())?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(invalid());
            }
            let bytes = std::fs::read(&expected).map_err(|_| invalid())?;
            if *size != Some(u64_to_i64(bytes.len() as u64, "种子大小")?)
                || checksum.as_deref() != Some(format!("{:x}", Sha256::digest(bytes)).as_str())
            {
                return Err(invalid());
            }
        }
        Ok(())
    })
    .await
    .map_err(|error| format!("核对私有 metadata 失败：{error}"))?
}

async fn persist_task_proxy_override_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    task: &DownloadTask,
) -> Result<(), String> {
    let task_id = u64_to_i64(task.id, "任务 ID")?;
    if task.source_type == DownloadTaskSourceType::Url
        && task.use_proxy
        && task.proxy_binding.source() == TaskProxySource::Override
    {
        let proxy_url = task
            .proxy_binding
            .effective_proxy_url()
            .ok_or_else(|| "兼容代理任务缺少私密代理覆盖".to_string())?;
        sqlx::query(
            r#"
            INSERT INTO task_proxy_overrides (task_id, proxy_url, updated_at)
            VALUES (?, ?, ?)
            ON CONFLICT(task_id) DO UPDATE SET
                proxy_url = excluded.proxy_url,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(task_id)
        .bind(proxy_url)
        .bind(u64_to_i64(task.updated_at, "更新时间")?)
        .execute(&mut **transaction)
        .await
        .map_err(|error| format!("保存任务私密代理覆盖失败：{}", error))?;
    } else {
        sqlx::query("DELETE FROM task_proxy_overrides WHERE task_id = ?")
            .bind(task_id)
            .execute(&mut **transaction)
            .await
            .map_err(|error| format!("清理任务私密代理覆盖失败：{}", error))?;
    }
    Ok(())
}

pub async fn persist_download_task_state(
    pool: &SqlitePool,
    task: &DownloadTask,
) -> Result<(), String> {
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| format!("启动持久化任务状态事务失败：{}", error))?;
    persist_download_task_state_in_transaction(&mut transaction, task).await?;
    transaction
        .commit()
        .await
        .map_err(|error| format!("提交持久化任务状态事务失败：{}", error))?;
    Ok(())
}

pub async fn persist_download_task_state_with_operation(
    pool: &SqlitePool,
    task: &DownloadTask,
    operation: &TaskOperation,
) -> Result<(), String> {
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| format!("启动持久化任务操作事务失败：{}", error))?;
    persist_download_task_state_in_transaction(&mut transaction, task).await?;
    update_task_operation_in_transaction(&mut transaction, operation).await?;
    transaction
        .commit()
        .await
        .map_err(|error| format!("提交持久化任务操作事务失败：{}", error))?;
    Ok(())
}

async fn persist_download_task_state_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    task: &DownloadTask,
) -> Result<(), String> {
    upsert_download_task_in_transaction(transaction, task).await?;

    match task.status {
        DownloadTaskStatus::Complete
        | DownloadTaskStatus::Paused
        | DownloadTaskStatus::Error
        | DownloadTaskStatus::Removed => {
            record_task_history_in_transaction(transaction, task, task.error_message.as_deref())
                .await?;
        }
        DownloadTaskStatus::Pending | DownloadTaskStatus::Active => {}
    }

    if task.status == DownloadTaskStatus::Error {
        record_task_error_in_transaction(transaction, task).await?;
    }

    Ok(())
}

pub async fn persist_download_task_states(
    pool: &SqlitePool,
    tasks: &[DownloadTask],
) -> Result<(), String> {
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| format!("启动保存任务快照事务失败：{error}"))?;
    for task in tasks {
        persist_download_task_state_in_transaction(&mut transaction, task).await?;
    }
    transaction
        .commit()
        .await
        .map_err(|error| format!("提交任务快照事务失败：{error}"))?;
    Ok(())
}

pub async fn list_download_tasks(pool: &SqlitePool) -> Result<Vec<DownloadTask>, String> {
    read_download_tasks(pool, false).await
}

pub(crate) async fn list_download_tasks_for_engine(
    pool: &SqlitePool,
    generation: crate::config::aria2::EngineGeneration,
) -> Result<Vec<DownloadTask>, String> {
    read_download_tasks(
        pool,
        generation == crate::config::aria2::EngineGeneration::NativeV2,
    )
    .await
}

async fn read_download_tasks(
    pool: &SqlitePool,
    isolate_legacy: bool,
) -> Result<Vec<DownloadTask>, String> {
    let has_generation: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pragma_table_info('download_tasks') WHERE name = 'engine_generation')")
        .fetch_one(pool).await.map_err(|_| "无法读取任务代际字段")?;
    let profile_proxy_url = get_download_proxy_config(pool)
        .await?
        .and_then(|config| config.proxy_url);
    let query = r#"
        SELECT download_tasks.id, engine_generation, url, source_type, file_name, save_dir, owned_task_dir, gid, status,
               total_length, completed_length, category, download_speed, error_code, error_message,
               file_path, metadata_torrent_path, files_deleted, selected_file_indexes,
               confirmation_required, use_proxy, proxy_source, task_proxy_overrides.proxy_url AS proxy_override_url,
               created_at, download_tasks.updated_at, removed_from_status
        FROM download_tasks
        LEFT JOIN task_proxy_overrides ON task_proxy_overrides.task_id = download_tasks.id
        ORDER BY created_at DESC, download_tasks.id DESC
        "#;
    let query = if has_generation {
        query.to_string()
    } else {
        let trusted: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = 7 AND name = 'web_auth_bootstrap_token')")
            .fetch_one(pool).await.map_err(|_| "无法核对旧任务来源")?;
        query
            .replace(
                "engine_generation",
                if trusted {
                    "'legacy-v1' AS engine_generation"
                } else {
                    "'unknown' AS engine_generation"
                },
            )
            .replace("removed_from_status", "NULL AS removed_from_status")
    };
    let query = if isolate_legacy {
        format!("SELECT * FROM ({query}) WHERE engine_generation != 'legacy-v1' OR engine_generation IS NULL")
    } else {
        query
    };
    let rows = sqlx::query(&query)
        .fetch_all(pool)
        .await
        .map_err(|error| format!("读取下载任务失败：{}", error))?;

    let mut tasks = rows
        .into_iter()
        .map(|row| row_to_task(row, profile_proxy_url.as_deref()))
        .collect::<Result<Vec<_>, _>>()?;
    if isolate_legacy
        && tasks
            .iter()
            .any(|task| task.engine_generation != crate::config::aria2::EngineGeneration::NativeV2)
    {
        return Err("任务代际无法核对，已保留状态".to_string());
    }
    if !has_generation {
        return Ok(tasks);
    }
    let files = sqlx::query(if isolate_legacy {
        "SELECT task_id, file_index, path, name, length, completed_length, selected FROM task_file_snapshots WHERE NOT EXISTS (SELECT 1 FROM download_tasks WHERE id = task_id AND engine_generation = 'legacy-v1') ORDER BY task_id, file_index"
    } else {
        "SELECT task_id, file_index, path, name, length, completed_length, selected FROM task_file_snapshots ORDER BY task_id, file_index"
    })
        .fetch_all(pool)
        .await
        .map_err(|error| format!("读取任务文件快照失败：{error}"))?;
    let mut by_task = std::collections::HashMap::<u64, Vec<DownloadTaskFile>>::new();
    for row in files {
        let index = u32::try_from(get::<i64>(&row, "file_index")?)
            .map_err(|_| "文件索引超出范围".to_string())?;
        by_task
            .entry(i64_to_u64(get(&row, "task_id")?, "任务 ID")?)
            .or_default()
            .push(DownloadTaskFile {
                index,
                path: get(&row, "path")?,
                name: get(&row, "name")?,
                length: i64_to_u64(get(&row, "length")?, "文件大小")?,
                completed_length: i64_to_u64(get(&row, "completed_length")?, "文件有效进度")?,
                selected: get(&row, "selected")?,
            });
    }
    for task in &mut tasks {
        task.files = by_task.remove(&task.id).unwrap_or_default();
        if task.engine_generation == crate::config::aria2::EngineGeneration::NativeV2 {
            let root = Path::new(crate::tasks::files::task_download_dir(task)).to_path_buf();
            for file in &mut task.files {
                let relative = Path::new(&file.path);
                if relative.as_os_str().is_empty()
                    || relative
                        .components()
                        .any(|part| !matches!(part, std::path::Component::Normal(_)))
                {
                    return Err("原生任务文件快照路径不安全".to_string());
                }
                file.path = root.join(relative).display().to_string();
            }
        }
    }
    if isolate_legacy && !by_task.is_empty() {
        return Err("任务文件快照归属无法核对，已保留状态".to_string());
    }
    Ok(tasks)
}

pub async fn max_download_task_id(pool: &SqlitePool) -> Result<u64, String> {
    let max_id: Option<i64> = sqlx::query_scalar(
        "SELECT MAX(id) FROM (SELECT id FROM download_tasks UNION ALL SELECT task_id AS id FROM task_operations)",
    )
        .fetch_one(pool)
        .await
        .map_err(|error| format!("读取最大任务 ID 失败：{}", error))?;

    Ok(max_id.unwrap_or_default().max(0) as u64)
}

pub async fn delete_download_task_record(pool: &SqlitePool, task_id: u64) -> Result<bool, String> {
    let task_id = u64_to_i64(task_id, "任务 ID")?;
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| format!("启动删除任务记录事务失败：{}", error))?;

    let deleted = delete_download_task_record_in_transaction(&mut transaction, task_id).await?;

    transaction
        .commit()
        .await
        .map_err(|error| format!("提交删除任务记录事务失败：{}", error))?;

    Ok(deleted)
}

pub async fn delete_download_task_record_with_operation(
    pool: &SqlitePool,
    task_id: u64,
    operation: &TaskOperation,
) -> Result<bool, String> {
    let task_id = u64_to_i64(task_id, "任务 ID")?;
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| format!("启动删除任务记录事务失败：{}", error))?;

    let deleted = delete_download_task_record_in_transaction(&mut transaction, task_id).await?;
    if !deleted {
        return Ok(false);
    }
    update_task_operation_in_transaction(&mut transaction, operation).await?;

    transaction
        .commit()
        .await
        .map_err(|error| format!("提交删除任务记录事务失败：{}", error))?;

    Ok(deleted)
}

async fn delete_download_task_record_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    task_id: i64,
) -> Result<bool, String> {
    sqlx::query("DELETE FROM task_history WHERE task_id = ?")
        .bind(task_id)
        .execute(&mut **transaction)
        .await
        .map_err(|error| format!("删除任务历史失败：{}", error))?;
    sqlx::query("DELETE FROM task_errors WHERE task_id = ?")
        .bind(task_id)
        .execute(&mut **transaction)
        .await
        .map_err(|error| format!("删除任务错误记录失败：{}", error))?;
    sqlx::query("DELETE FROM task_proxy_overrides WHERE task_id = ?")
        .bind(task_id)
        .execute(&mut **transaction)
        .await
        .map_err(|error| format!("删除任务私密代理覆盖失败：{}", error))?;
    for table in ["task_file_snapshots", "task_metadata_refs"] {
        sqlx::query(&format!("DELETE FROM {table} WHERE task_id = ?"))
            .bind(task_id)
            .execute(&mut **transaction)
            .await
            .map_err(|error| format!("删除任务状态引用失败：{error}"))?;
    }
    let result = sqlx::query("DELETE FROM download_tasks WHERE id = ?")
        .bind(task_id)
        .execute(&mut **transaction)
        .await
        .map_err(|error| format!("删除下载任务记录失败：{}", error))?;

    Ok(result.rows_affected() > 0)
}

pub async fn record_task_history(
    pool: &SqlitePool,
    task: &DownloadTask,
    message: Option<&str>,
) -> Result<(), String> {
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| format!("启动保存任务历史事务失败：{}", error))?;
    record_task_history_in_transaction(&mut transaction, task, message).await?;
    transaction
        .commit()
        .await
        .map_err(|error| format!("提交保存任务历史事务失败：{}", error))?;
    Ok(())
}

async fn record_task_history_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    task: &DownloadTask,
    message: Option<&str>,
) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT INTO task_history (task_id, status, message, created_at)
        SELECT ?, ?, ?, ?
        WHERE NOT EXISTS (
            SELECT 1
            FROM task_history
            WHERE task_id = ?
              AND status = ?
            ORDER BY created_at DESC
            LIMIT 1
        )
        "#,
    )
    .bind(u64_to_i64(task.id, "任务 ID")?)
    .bind(task.status.as_storage_value())
    .bind(message)
    .bind(u64_to_i64(task.updated_at, "更新时间")?)
    .bind(u64_to_i64(task.id, "任务 ID")?)
    .bind(task.status.as_storage_value())
    .execute(&mut **transaction)
    .await
    .map_err(|error| format!("保存任务历史失败：{}", error))?;

    Ok(())
}

pub async fn record_task_error(pool: &SqlitePool, task: &DownloadTask) -> Result<(), String> {
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| format!("启动保存任务错误事务失败：{}", error))?;
    record_task_error_in_transaction(&mut transaction, task).await?;
    transaction
        .commit()
        .await
        .map_err(|error| format!("提交保存任务错误事务失败：{}", error))?;
    Ok(())
}

async fn record_task_error_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    task: &DownloadTask,
) -> Result<(), String> {
    let Some(message) = task
        .error_message
        .as_deref()
        .filter(|message| !message.trim().is_empty())
    else {
        return Ok(());
    };

    sqlx::query(
        r#"
        INSERT INTO task_errors (task_id, error_code, error_message, created_at)
        SELECT ?, ?, ?, ?
        WHERE NOT EXISTS (
            SELECT 1
            FROM task_errors
            WHERE task_id = ?
              AND COALESCE(error_code, '') = COALESCE(?, '')
              AND error_message = ?
            LIMIT 1
        )
        "#,
    )
    .bind(u64_to_i64(task.id, "任务 ID")?)
    .bind(&task.error_code)
    .bind(message)
    .bind(u64_to_i64(task.updated_at, "更新时间")?)
    .bind(u64_to_i64(task.id, "任务 ID")?)
    .bind(&task.error_code)
    .bind(message)
    .execute(&mut **transaction)
    .await
    .map_err(|error| format!("保存任务错误记录失败：{}", error))?;

    Ok(())
}

fn row_to_task(
    row: sqlx::sqlite::SqliteRow,
    profile_proxy_url: Option<&str>,
) -> Result<DownloadTask, String> {
    let status: String = get(&row, "status")?;
    let source_type: String = get(&row, "source_type")?;
    let generation = crate::config::aria2::EngineGeneration::from_storage_value(&get::<String>(
        &row,
        "engine_generation",
    )?);
    if generation == crate::config::aria2::EngineGeneration::NativeV2
        && (!matches!(
            status.as_str(),
            "pending" | "active" | "paused" | "complete" | "error" | "removed"
        ) || !matches!(source_type.as_str(), "url" | "magnet" | "torrent"))
    {
        return Err("原生任务记录包含未知状态或来源，已保留并拒绝恢复".to_string());
    }
    let use_proxy = source_type == "url" && get::<i64>(&row, "use_proxy")? != 0;
    let proxy_source = if use_proxy {
        TaskProxySource::from_storage_value(&get::<String>(&row, "proxy_source")?)
    } else {
        TaskProxySource::Profile
    };
    let effective_proxy_url = if use_proxy {
        match proxy_source {
            TaskProxySource::Profile => profile_proxy_url.map(str::to_owned),
            TaskProxySource::Override => get(&row, "proxy_override_url")?,
        }
    } else {
        None
    };
    Ok(DownloadTask {
        id: i64_to_u64(get(&row, "id")?, "任务 ID")?,
        engine_generation: generation,
        url: get(&row, "url")?,
        source_type: DownloadTaskSourceType::from_storage_value(&source_type),
        file_name: get(&row, "file_name")?,
        save_dir: get(&row, "save_dir")?,
        owned_task_dir: get(&row, "owned_task_dir")?,
        category: get(&row, "category")?,
        gid: get(&row, "gid")?,
        status: DownloadTaskStatus::from_storage_value(&status),
        removed_from_status: get::<Option<String>>(&row, "removed_from_status")?
            .map(|status| DownloadTaskStatus::from_storage_value(&status)),
        total_length: i64_to_u64(get(&row, "total_length")?, "总大小")?,
        completed_length: i64_to_u64(get(&row, "completed_length")?, "已下载大小")?,
        download_speed: i64_to_u64(get(&row, "download_speed")?, "下载速度")?,
        error_code: get(&row, "error_code")?,
        error_message: get(&row, "error_message")?,
        file_path: get(&row, "file_path")?,
        use_proxy,
        proxy_binding: TaskProxyBinding::from_persisted(proxy_source, effective_proxy_url),
        metadata_torrent_path: get(&row, "metadata_torrent_path")?,
        files_deleted: get::<i64>(&row, "files_deleted")? != 0,
        selected_file_indexes: serde_json::from_str(&get::<String>(&row, "selected_file_indexes")?)
            .map_err(|error| format!("读取任务文件选择字段失败：{}", error))?,
        confirmation_required: get::<i64>(&row, "confirmation_required")? != 0,
        files: Vec::new(),
        created_at: i64_to_u64(get(&row, "created_at")?, "创建时间")?,
        updated_at: i64_to_u64(get(&row, "updated_at")?, "更新时间")?,
    })
}

fn get<'row, T>(row: &'row sqlx::sqlite::SqliteRow, column: &str) -> Result<T, String>
where
    T: Decode<'row, Sqlite> + Type<Sqlite>,
{
    row.try_get(column)
        .map_err(|error| format!("读取下载任务字段 {} 失败：{}", column, error))
}

fn u64_to_i64(value: u64, label: &str) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("{} 超出 SQLite INTEGER 范围", label))
}

fn i64_to_u64(value: i64, label: &str) -> Result<u64, String> {
    u64::try_from(value).map_err(|_| format!("{} 不能为负数", label))
}

#[cfg(test)]
mod tests;
