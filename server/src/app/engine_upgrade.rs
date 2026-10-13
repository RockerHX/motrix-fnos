use super::compatibility::{
    inspect_legacy_state, read_private_text, regular_file_exists, AppCompatibility,
};
use crate::auth::ServerProcessLock;
use crate::config::aria2::EngineGeneration;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

const COMPLETE_MARKER: &str = "engine/native-v2/migration-complete.json";
const PENDING_MARKER: &str = "engine-upgrade-pending.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MigrationMarker {
    generation: EngineGeneration,
    backup: Option<PathBuf>,
    backup_hash: Option<String>,
    archive_hash: Option<String>,
}

pub(crate) async fn startup_compatibility(
    root: &Path,
    generation: EngineGeneration,
) -> AppCompatibility {
    if generation == EngineGeneration::LegacyV1 {
        return AppCompatibility::normal(generation);
    }
    if generation == EngineGeneration::Unknown {
        return AppCompatibility::restricted(generation, "legacy_state_invalid");
    }
    let inspection = match inspect_legacy_state(root).await {
        Ok(inspection) => inspection,
        Err(status) => return status,
    };
    match validate_complete_marker(root) {
        Ok(true) => {
            // 完成标记已经证明新状态完整；上次只来得及写标记、未删 pending 时可安全清理。
            if root.join(PENDING_MARKER).exists() {
                let _ = fs::remove_file(root.join(PENDING_MARKER));
            }
            AppCompatibility::normal(generation)
        }
        Err(_) => AppCompatibility::restricted(generation, "engine_migration_incomplete"),
        Ok(false) if inspection.has_legacy_state || root.join(PENDING_MARKER).exists() => {
            AppCompatibility::restricted(generation, "engine_migration_incomplete")
        }
        Ok(false) => {
            let marker = MigrationMarker {
                generation,
                backup: None,
                backup_hash: None,
                archive_hash: None,
            };
            match write_marker(root, COMPLETE_MARKER, &marker) {
                Ok(()) => AppCompatibility::normal(generation),
                Err(_) => AppCompatibility::restricted(generation, "engine_migration_incomplete"),
            }
        }
    }
}

pub(crate) async fn run_upgrade(root: &Path, apply: bool) -> Result<(), String> {
    let _lock = ServerProcessLock::acquire(root)?;
    ensure_processes_stopped(root)?;
    inspect_legacy_state(root).await.map_err(|status| {
        format!(
            "{}：{}",
            status.reason_code.unwrap_or_default(),
            status.message
        )
    })?;
    if validate_complete_marker(root)? {
        return Ok(());
    }
    if !apply {
        return Ok(());
    }

    let mut marker = match read_private_text(&root.join(PENDING_MARKER))? {
        Some(content) => serde_json::from_str::<MigrationMarker>(&content)
            .map_err(|_| "迁移中断记录无效，已保留数据")?,
        None => {
            let parent = root.parent().ok_or("应用数据目录缺少父目录")?;
            let name = root
                .file_name()
                .ok_or("应用数据目录名称无效")?
                .to_string_lossy();
            let backup = parent.join(format!(
                "{name}.motrix-pre-native-v2-{}",
                crate::aria2::generate_rpc_secret()
            ));
            copy_private_tree(root, &backup)?;
            let marker = MigrationMarker {
                generation: EngineGeneration::NativeV2,
                backup_hash: Some(tree_hash(&backup)?),
                backup: Some(backup),
                archive_hash: None,
            };
            write_marker(root, PENDING_MARKER, &marker)?;
            marker
        }
    };
    validate_backup(root, &marker)?;
    let archive = crate::state::ensure_private_directory(root, Path::new("engine/legacy-v1"))?;
    for relative in ["aria2", "magnet-metadata", "task-metadata"] {
        let source = root.join(relative);
        if source.exists() {
            copy_private_tree(&source, &archive.join(relative))?;
        }
    }
    marker.archive_hash = Some(tree_hash(&archive)?);
    // schema 迁移幂等；完成标记只在一致备份、归档及新状态隔离均成功之后写入。
    let database =
        crate::database::connect_database(root.join(crate::database::DATABASE_FILE_NAME)).await?;
    database.pool.close().await;
    crate::state::ensure_private_directory(root, Path::new("engine/native-v2/state"))?;
    crate::state::ensure_private_directory(root, Path::new("engine/native-v2/session"))?;
    write_marker(root, COMPLETE_MARKER, &marker)?;
    fs::remove_file(root.join(PENDING_MARKER)).map_err(|_| "迁移已完成，但中断记录无法清理")?;
    Ok(())
}

fn ensure_processes_stopped(root: &Path) -> Result<(), String> {
    if let Some(content) = read_private_text(&root.join("run/motrix-fnos-server.pid"))? {
        let pid = content
            .trim()
            .parse::<i32>()
            .map_err(|_| "服务 PID 无效，拒绝升级")?;
        ensure_pid_stopped(pid)?;
    }
    if let Some(runtime) =
        crate::state::read_aria2_runtime_record(&root.join("aria2-runtime.json"))?
    {
        ensure_pid_stopped(runtime.pid.try_into().map_err(|_| "引擎 PID 无效")?)?;
    }
    Ok(())
}

fn ensure_pid_stopped(pid: i32) -> Result<(), String> {
    if pid <= 0 {
        return Err("PID 无效，拒绝升级".to_string());
    }
    if unsafe { libc::kill(pid, 0) } == 0
        || std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
    {
        return Err("所属服务或引擎尚未确认退出，请先停止应用".to_string());
    }
    Ok(())
}

fn validate_complete_marker(root: &Path) -> Result<bool, String> {
    let Some(content) = read_private_text(&root.join(COMPLETE_MARKER))? else {
        return Ok(false);
    };
    let marker: MigrationMarker = serde_json::from_str(&content).map_err(|_| "迁移完成标记无效")?;
    if marker.generation != EngineGeneration::NativeV2 {
        return Err("迁移代际无效".to_string());
    }
    validate_backup(root, &marker)?;
    if let Some(expected) = marker.archive_hash {
        if tree_hash(&root.join("engine/legacy-v1"))? != expected {
            return Err("旧状态归档校验失败".to_string());
        }
    }
    Ok(true)
}

fn validate_backup(root: &Path, marker: &MigrationMarker) -> Result<(), String> {
    match (&marker.backup, &marker.backup_hash) {
        (Some(path), Some(expected)) if path.parent() == root.parent() && path != root => {
            if tree_hash(path)? != *expected {
                return Err("升级备份校验失败".to_string());
            }
            Ok(())
        }
        (None, None) => Ok(()),
        _ => Err("升级备份关联无效".to_string()),
    }
}

fn write_marker(root: &Path, relative: &str, marker: &MigrationMarker) -> Result<(), String> {
    let path = root.join(relative);
    let parent = Path::new(relative).parent().unwrap();
    if !parent.as_os_str().is_empty() {
        crate::state::ensure_private_directory(root, parent)?;
    }
    let temporary = path.with_extension(format!("tmp-{}", crate::aria2::generate_rpc_secret()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&temporary)?;
        file.write_all(&serde_json::to_vec(marker)?)?;
        file.sync_all()?;
        fs::rename(&temporary, &path)?;
        fs::File::open(path.parent().unwrap())?.sync_all()
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result.map_err(|_: std::io::Error| "无法提交迁移标记".to_string())
}

fn copy_private_tree(source: &Path, destination: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(source).map_err(|_| "无法读取升级备份源")?;
    if metadata.is_file() {
        regular_file_exists(source)?;
        if regular_file_exists(destination)? {
            if fs::read(source).map_err(|_| "无法读取备份源")?
                != fs::read(destination).map_err(|_| "无法读取已归档状态")?
            {
                return Err("已归档状态与源不一致，拒绝覆盖".to_string());
            }
        } else {
            let mut output = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(destination)
                .map_err(|_| "无法创建备份文件")?;
            std::io::copy(
                &mut fs::File::open(source).map_err(|_| "无法读取备份源")?,
                &mut output,
            )
            .map_err(|_| "备份失败")?;
            output.sync_all().map_err(|_| "备份未能落盘")?;
        }
    } else if metadata.is_dir() && !metadata.file_type().is_symlink() {
        match fs::symlink_metadata(destination) {
            Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {}
            Ok(_) => return Err("备份目录类型无效".to_string()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                use std::os::unix::fs::DirBuilderExt;
                fs::DirBuilder::new()
                    .mode(0o700)
                    .create(destination)
                    .map_err(|_| "无法创建备份目录")?;
            }
            Err(_) => return Err("无法读取备份目录".to_string()),
        }
        for entry in fs::read_dir(source).map_err(|_| "无法扫描备份源")? {
            let entry = entry.map_err(|_| "无法读取备份条目")?;
            copy_private_tree(&entry.path(), &destination.join(entry.file_name()))?;
        }
        fs::File::open(destination)
            .and_then(|file| file.sync_all())
            .map_err(|_| "备份目录未能落盘")?;
    } else {
        return Err("备份源包含链接或特殊文件，拒绝升级".to_string());
    }
    Ok(())
}

fn tree_hash(root: &Path) -> Result<String, String> {
    let mut digest = Sha256::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        let metadata = fs::symlink_metadata(&path).map_err(|_| "无法校验备份条目")?;
        digest.update(
            path.strip_prefix(root)
                .map_err(|_| "备份路径无效")?
                .as_os_str()
                .as_encoded_bytes(),
        );
        digest.update([0]);
        if metadata.is_file() {
            regular_file_exists(&path)?;
            digest.update(b"file");
            let mut file = fs::File::open(&path).map_err(|_| "无法校验备份文件")?;
            std::io::copy(&mut file, &mut digest).map_err(|_| "无法计算备份校验和")?;
        } else if metadata.is_dir() && !metadata.file_type().is_symlink() {
            digest.update(b"dir");
            let mut entries = fs::read_dir(path)
                .map_err(|_| "无法校验备份目录")?
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "无法校验备份条目")?;
            entries.sort();
            pending.extend(entries.into_iter().rev());
        } else {
            return Err("备份包含链接或特殊文件".to_string());
        }
    }
    Ok(format!("{:x}", digest.finalize()))
}
