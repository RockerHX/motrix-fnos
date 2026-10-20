use motrix_fnos_server::app::{bootstrap_http_app_state, ServerRuntimeConfig};
use motrix_fnos_server::auth::{AuthService, ServerProcessLock};
use motrix_fnos_server::database::{connect_database, DATABASE_FILE_NAME};
use motrix_fnos_server::settings::service::save_json_rpc_token;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

const COMPLETE: &str = "engine/native-v2/migration-complete.json";
const PENDING: &str = "engine-upgrade-pending.json";

struct Installation(PathBuf);

impl Installation {
    async fn legacy() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "motrix-upgrade-cli-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).unwrap();
        let installation = Self(root);
        fs::create_dir_all(installation.0.join("cmd")).unwrap();
        fs::create_dir_all(installation.0.join("target/bin")).unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../packaging/fnos/cmd");
        for script in [
            "common.sh",
            "upgrade_init",
            "upgrade_callback",
            "uninstall_callback",
        ] {
            symlink(source.join(script), installation.0.join("cmd").join(script)).unwrap();
        }
        symlink(
            env!("CARGO_BIN_EXE_motrix-fnos-server"),
            installation.0.join("cmd/upgrade-checker"),
        )
        .unwrap();
        fs::write(
            installation.0.join("target/bin/engine-generation"),
            "native-v2\n",
        )
        .unwrap();
        fs::write(
            installation.0.join("target/bin/aria2-next"),
            "#!/bin/sh\n[ \"$1\" = --version ] || exit 99\nprintf 'aria2 version 2.8.6\\n'\n",
        )
        .unwrap();
        fs::set_permissions(
            installation.0.join("target/bin/aria2-next"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        let database = connect_database(installation.data().join(DATABASE_FILE_NAME))
            .await
            .unwrap();
        AuthService::new(database.pool.clone())
            .initialize_password("preserved management password")
            .await
            .unwrap();
        save_json_rpc_token(&database.pool, "preserved-public-token")
            .await
            .unwrap();
        sqlx::query("INSERT INTO app_config VALUES ('jsonrpc_lan', ?, 1)")
            .bind(
                r#"{"enabled":true,"token":"preserved-lan-token","allowSharedAddressSpace":false}"#,
            )
            .execute(&database.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO app_config VALUES ('download_proxy', ?, 1)")
            .bind(
                r#"{"proxyUrl":"http://user:private@127.0.0.1:7890/","revision":1,"updatedAt":1}"#,
            )
            .execute(&database.pool)
            .await
            .unwrap();
        let downloads = installation.0.join("downloads");
        fs::create_dir(&downloads).unwrap();
        fs::write(downloads.join("keep.bin"), b"external download payload").unwrap();
        for (id, status, gid) in [
            (1, "complete", "0000000000000001"),
            (2, "removed", "0000000000000002"),
        ] {
            sqlx::query("INSERT INTO download_tasks (id, url, file_name, save_dir, gid, status, created_at, updated_at) VALUES (?, 'https://fixture.invalid/keep.bin', 'keep.bin', ?, ?, ?, 1, 1)")
                .bind(id).bind(downloads.to_str().unwrap()).bind(gid).bind(status)
                .execute(&database.pool).await.unwrap();
        }
        // 还原可信旧 schema，确保 CLI 在读取检查阶段不会提前执行 v8 迁移。
        for statement in [
            "DROP TABLE task_file_snapshots",
            "DROP TABLE task_metadata_refs",
            "ALTER TABLE download_tasks DROP COLUMN engine_generation",
            "ALTER TABLE download_tasks DROP COLUMN metadata_state",
            "ALTER TABLE download_tasks DROP COLUMN cleanup_state",
            "ALTER TABLE download_tasks DROP COLUMN removed_from_status",
            "DELETE FROM schema_migrations WHERE version = 8",
        ] {
            sqlx::query(statement)
                .execute(&database.pool)
                .await
                .unwrap();
        }
        database.pool.close().await;
        for (file, contents) in [
            (
                "aria2/aria2.session",
                b"https://fixture.invalid/keep.bin\n gid=0000000000000002\n".as_slice(),
            ),
            ("aria2/state/resume.bin", b"legacy engine state"),
            ("task-metadata/2.torrent", b"preserved torrent"),
            ("magnet-metadata/2/metadata.bin", b"preserved metadata"),
            ("accessible-paths.json", b"{\"paths\":[]}"),
            ("run/motrix-fnos-server.lock", b""),
            ("logs/server.log", b""),
        ] {
            let target = installation.data().join(file);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, contents).unwrap();
        }
        installation
    }

    fn data(&self) -> PathBuf {
        self.0.join("var")
    }

    fn cli(&self, command: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_motrix-fnos-server"))
            .arg(command)
            .env("MOTRIX_FNOS_APP_DATA_DIR", self.data())
            .output()
            .unwrap()
    }

    fn hook(&self, script: &str) -> Output {
        Command::new("/bin/sh")
            .arg(self.0.join("cmd").join(script))
            .env("TRIM_APPDEST", self.0.join("target"))
            .env("TRIM_PKGVAR", self.data())
            .env("MOTRIX_FNOS_APP_DATA_DIR", self.data())
            .env("MOTRIX_FNOS_DELETE_APP_DATA", "false")
            .env("TRIM_TEMP_LOGFILE", self.0.join("platform-error"))
            .output()
            .unwrap()
    }

    async fn pool_at(root: &Path) -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::new().filename(root.join(DATABASE_FILE_NAME)))
            .await
            .unwrap()
    }

    async fn execute(&self, sql: &str) {
        let pool = Self::pool_at(&self.data()).await;
        sqlx::query(sql).execute(&pool).await.unwrap();
        pool.close().await;
    }

    fn backups(&self) -> Vec<PathBuf> {
        fs::read_dir(&self.0)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|entry| {
                entry
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("var.motrix-pre-native-v2-")
            })
            .collect()
    }

    async fn restricted(&self) {
        let state = bootstrap_http_app_state(&ServerRuntimeConfig {
            app_data_dir: self.data(),
            database_path: self.data().join(DATABASE_FILE_NAME),
            accessible_paths_path: self.data().join("accessible-paths.json"),
            aria2_path: Some(self.0.join("target/bin/aria2-next")),
            http_addr: "127.0.0.1:0".parse().unwrap(),
            jsonrpc_addr: "127.0.0.1:0".parse().unwrap(),
            lan_jsonrpc_addr: "127.0.0.1:0".parse().unwrap(),
            trusted_proxy_ips: Vec::new(),
        })
        .await
        .unwrap();
        assert_eq!(state.compatibility_snapshot().mode, "restricted");
        assert!(state.ensure_download_available().is_err());
        AuthService::new(state.core.database.pool.clone())
            .verify_password("preserved management password")
            .await
            .unwrap();
        assert!(!self.data().join("aria2-runtime.json").exists());
        state.core.database.pool.close().await;
    }
}

impl Drop for Installation {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn installed_hooks_preserve_legacy_data_and_backup_restores_the_old_schema() {
    let installation = Installation::legacy().await;
    let pool = Installation::pool_at(&installation.data()).await;
    let settings: Vec<(String, String)> =
        sqlx::query_as("SELECT key, value FROM app_config ORDER BY key")
            .fetch_all(&pool)
            .await
            .unwrap();
    let auth_before = AuthService::new(pool.clone()).state().await.unwrap();
    let tasks_before: String = sqlx::query_scalar(
        "SELECT json_group_array(json_array(id, url, status, gid, save_dir)) FROM (SELECT * FROM download_tasks ORDER BY id)",
    ).fetch_one(&pool).await.unwrap();
    pool.close().await;
    assert_success(installation.hook("upgrade_init"));
    let before = files(&installation.data());
    assert_success(installation.hook("upgrade_init"));
    assert_eq!(
        files(&installation.data()),
        before,
        "检查不得迁移或改写旧状态"
    );
    assert!(installation.backups().is_empty());

    assert_success(installation.hook("upgrade_callback"));
    assert_eq!(installation.backups().len(), 1);
    let backup = &installation.backups()[0];
    let saved = files(backup);
    assert_eq!(saved, before, "全量备份必须与升级前数据逐文件一致");
    for relative in ["aria2", "task-metadata", "magnet-metadata"] {
        assert_eq!(
            files(&installation.data().join(relative)),
            files(&installation.data().join("engine/legacy-v1").join(relative))
        );
    }
    for directory in ["engine/native-v2/state", "engine/native-v2/session"] {
        assert!(fs::read_dir(installation.data().join(directory))
            .unwrap()
            .next()
            .is_none());
    }
    assert!(!installation.data().join(PENDING).exists());
    let marker = fs::read(installation.data().join(COMPLETE)).unwrap();
    let pool = Installation::pool_at(&installation.data()).await;
    let tasks_after: String = sqlx::query_scalar(
        "SELECT json_group_array(json_array(id, url, status, gid, save_dir)) FROM (SELECT * FROM download_tasks ORDER BY id)",
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(
        tasks_after, tasks_before,
        "任务身份、状态和文件目录必须保留"
    );
    let settings_after: Vec<(String, String)> =
        sqlx::query_as("SELECT key, value FROM app_config ORDER BY key")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        settings_after, settings,
        "设置、代理及两个 RPC Token 必须保留"
    );
    assert_eq!(
        AuthService::new(pool.clone()).state().await.unwrap(),
        auth_before
    );
    let generations: Vec<String> =
        sqlx::query_scalar("SELECT engine_generation FROM download_tasks ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(generations, ["legacy-v1", "legacy-v1"]);
    AuthService::new(pool.clone())
        .verify_password("preserved management password")
        .await
        .unwrap();
    pool.close().await;
    assert_success(installation.hook("upgrade_callback"));
    assert_eq!(installation.backups().len(), 1, "重复执行不能重复创建备份");
    assert_eq!(
        fs::read(installation.data().join(COMPLETE)).unwrap(),
        marker
    );
    assert_eq!(files(backup), saved);

    let restored = installation.0.join("restored");
    copy_tree(backup, &restored);
    assert_eq!(files(&restored), saved);
    let pool = Installation::pool_at(&restored).await;
    let version: i64 = sqlx::query_scalar("SELECT MAX(version) FROM schema_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(version, 7, "回滚必须恢复升级前 schema");
    AuthService::new(pool.clone())
        .verify_password("preserved management password")
        .await
        .unwrap();
    pool.close().await;
    assert_eq!(
        fs::read(installation.0.join("downloads/keep.bin")).unwrap(),
        b"external download payload"
    );
    let before_uninstall = files(&installation.data());
    assert_success(installation.hook("uninstall_callback"));
    assert_eq!(
        files(&installation.data()),
        before_uninstall,
        "默认卸载必须保留全部私有数据"
    );
}

#[tokio::test]
async fn checker_rejects_unfinished_tasks_unknown_operations_and_unassociated_session() {
    let installation = Installation::legacy().await;
    for status in ["active", "pending", "paused", "error"] {
        installation
            .execute(&format!(
                "UPDATE download_tasks SET status = '{status}' WHERE id = 1"
            ))
            .await;
        let before = protected_files(&installation.data());
        for command in ["engine-upgrade-check", "engine-upgrade-migrate"] {
            assert_failure(installation.cli(command), "legacy_tasks_unfinished");
        }
        assert_eq!(protected_files(&installation.data()), before);
    }
    installation.restricted().await;
    installation
        .execute("UPDATE download_tasks SET status = 'complete' WHERE id = 1")
        .await;
    installation.execute("INSERT INTO task_operations VALUES ('unknown', 2, 'delete', 'file_cleanup_pending', '{}', NULL, 'failed', 1, 1)").await;
    assert_failure(
        installation.cli("engine-upgrade-check"),
        "legacy_operations_unresolved",
    );
    assert!(!installation.hook("upgrade_init").status.success());
    let message = fs::read_to_string(installation.0.join("platform-error")).unwrap();
    assert!(message.contains("原数据"));
    assert!(!message.contains("private"));
    installation.execute("DELETE FROM task_operations").await;
    fs::write(
        installation.data().join("aria2/aria2.session"),
        b"https://fixture.invalid/unknown\n gid=1111111111111111\n",
    )
    .unwrap();
    let before = protected_files(&installation.data());
    assert_failure(
        installation.cli("engine-upgrade-migrate"),
        "legacy_state_invalid",
    );
    assert_eq!(protected_files(&installation.data()), before);
    assert!(installation.backups().is_empty());
    assert!(!installation.data().join(COMPLETE).exists());
}

#[tokio::test]
async fn migration_interruption_retries_the_same_backup_and_detects_tampering() {
    let installation = Installation::legacy().await;
    let blocked = installation.data().join("engine/native-v2/state");
    fs::create_dir_all(blocked.parent().unwrap()).unwrap();
    fs::write(&blocked, "blocked directory").unwrap();
    assert!(!installation.hook("upgrade_callback").status.success());
    assert!(installation.data().join(PENDING).exists());
    assert!(!installation.data().join(COMPLETE).exists());
    assert_eq!(installation.backups().len(), 1);
    let backup = installation.backups()[0].clone();
    let before = files(&backup);
    installation.restricted().await;
    fs::remove_file(blocked).unwrap();
    assert_success(installation.hook("upgrade_callback"));
    let marker: Value =
        serde_json::from_slice(&fs::read(installation.data().join(COMPLETE)).unwrap()).unwrap();
    assert_eq!(Path::new(marker["backup"].as_str().unwrap()), backup);
    assert_eq!(installation.backups().len(), 1);
    assert_eq!(files(&backup), before);
    assert!(!installation.data().join(PENDING).exists());
    let archive = installation
        .data()
        .join("engine/legacy-v1/aria2/state/resume.bin");
    fs::write(&archive, "corrupted archive").unwrap();
    assert_failure(installation.cli("engine-upgrade-check"), "归档校验失败");
    installation.restricted().await;
    fs::write(archive, b"legacy engine state").unwrap();
    fs::write(backup.join("task-metadata/2.torrent"), "corrupted backup").unwrap();
    assert_failure(installation.cli("engine-upgrade-migrate"), "备份校验失败");
    installation.restricted().await;
    assert_eq!(
        fs::read(installation.data().join("task-metadata/2.torrent")).unwrap(),
        b"preserved torrent"
    );
}

#[tokio::test]
async fn checker_requires_exclusive_lock_stopped_processes_and_valid_private_state() {
    let installation = Installation::legacy().await;
    let lock = ServerProcessLock::acquire(&installation.data()).unwrap();
    assert_failure(installation.cli("engine-upgrade-migrate"), "正在运行");
    drop(lock);
    let pid = installation.data().join("run/motrix-fnos-server.pid");
    fs::write(&pid, std::process::id().to_string()).unwrap();
    assert_failure(installation.cli("engine-upgrade-check"), "先停止应用");
    fs::write(&pid, "invalid pid").unwrap();
    assert_failure(installation.cli("engine-upgrade-migrate"), "PID 无效");
    fs::remove_file(pid).unwrap();
    installation
        .execute("DELETE FROM schema_migrations WHERE version = 7")
        .await;
    assert_failure(
        installation.cli("engine-upgrade-check"),
        "legacy_state_invalid",
    );
    assert!(installation.backups().is_empty());
}

fn assert_success(output: Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_failure(output: Output, expected: &str) {
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(expected),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn files(root: &Path) -> BTreeMap<PathBuf, String> {
    let mut output = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                output.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    format!("{:x}", Sha256::digest(fs::read(path).unwrap())),
                );
            }
        }
    }
    output
}

fn protected_files(root: &Path) -> BTreeMap<PathBuf, String> {
    let mut snapshot = files(root);
    // SQLite 只读连接可能留下空 WAL 和共享内存；不得忽略包含未提交字节的 WAL。
    snapshot.remove(Path::new("motrix-fnos.sqlite-shm"));
    if let Some(hash) = snapshot.remove(Path::new("motrix-fnos.sqlite-wal")) {
        assert_eq!(hash, format!("{:x}", Sha256::digest([])));
    }
    snapshot
}

fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir_all(target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let destination = target.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).unwrap();
        }
    }
}
