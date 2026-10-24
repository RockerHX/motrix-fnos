use motrix_fnos_server::app::{bootstrap_http_app_state, HttpAppState, ServerRuntimeConfig};
use motrix_fnos_server::auth::AuthService;
use motrix_fnos_server::config::aria2::EngineGeneration;
use motrix_fnos_server::database::task_operations::begin_task_operation;
use motrix_fnos_server::database::{connect_database, DATABASE_FILE_NAME};
use motrix_fnos_server::settings::service::save_json_rpc_token;
use motrix_fnos_server::tasks::{TaskOperation, TaskOperationContext, TaskOperationType};
use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

struct Installation(PathBuf);

impl Installation {
    async fn legacy() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "motrix-engine-upgrade-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).unwrap();
        let installation = Self(root);
        fs::create_dir_all(installation.0.join("target/bin")).unwrap();
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
            (1, "paused", "0000000000000001"),
            (2, "removed", "0000000000000002"),
        ] {
            sqlx::query("INSERT INTO download_tasks (id, url, file_name, save_dir, gid, status, created_at, updated_at) VALUES (?, 'https://fixture.invalid/keep.bin', 'keep.bin', ?, ?, ?, 1, 1)")
                .bind(id).bind(downloads.to_str().unwrap()).bind(gid).bind(status)
                .execute(&database.pool).await.unwrap();
        }
        // 从旧 schema 启动，验证正常升级后仍保留旧记录。
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

    async fn bootstrap(&self) -> Arc<HttpAppState> {
        bootstrap_http_app_state(&ServerRuntimeConfig {
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
        .unwrap()
    }

    async fn pool(&self) -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new().filename(self.data().join(DATABASE_FILE_NAME)),
            )
            .await
            .unwrap()
    }
}

impl Drop for Installation {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn native_upgrade_starts_without_legacy_tasks_and_preserves_files_settings_and_operations() {
    let installation = Installation::legacy().await;
    let pool = installation.pool().await;
    let settings: Vec<(String, String)> =
        sqlx::query_as("SELECT key, value FROM app_config ORDER BY key")
            .fetch_all(&pool)
            .await
            .unwrap();
    let old_file = installation.0.join("downloads/keep.bin");
    let context = json!({"oldGid": null, "newGid": null, "taskSnapshot": null,
        "fileCleanupPaths": [old_file], "completedSideEffects": []});
    sqlx::query("INSERT INTO task_operations VALUES ('legacy-cleanup', 900, 'delete', 'file_cleanup_pending', ?, NULL, 'in_progress', 1, 1)")
        .bind(context.to_string()).execute(&pool).await.unwrap();
    pool.close().await;
    let old_session = fs::read(installation.data().join("aria2/aria2.session")).unwrap();

    for _ in 0..2 {
        let state = installation.bootstrap().await;
        assert_eq!(state.compatibility_snapshot().mode, "normal");
        state.ensure_download_available().unwrap();
        assert!(state.core.download_tasks.list().unwrap().is_empty());
        assert_eq!(state.core.next_task_id.load(Ordering::Relaxed), 901);
        state
            .auth
            .service
            .verify_password("preserved management password")
            .await
            .unwrap();
        let pool = &state.core.database.pool;
        let after: Vec<(String, String)> =
            sqlx::query_as("SELECT key, value FROM app_config ORDER BY key")
                .fetch_all(pool)
                .await
                .unwrap();
        assert_eq!(after, settings);
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM download_tasks WHERE engine_generation = 'legacy-v1'",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(count, 2);
        let status: String =
            sqlx::query_scalar("SELECT status FROM task_operations WHERE id = 'legacy-cleanup'")
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(status, "in_progress");
        assert_eq!(fs::read(&old_file).unwrap(), b"external download payload");
        assert_eq!(
            fs::read(installation.data().join("aria2/aria2.session")).unwrap(),
            old_session
        );
        assert!(!installation.data().join("aria2-runtime.json").exists());
        assert!(!installation
            .data()
            .join("engine/native-v2/migration-complete.json")
            .exists());
        assert!(!installation.0.join("cmd/upgrade-checker").exists());
        state.core.database.pool.close().await;
    }
    assert!(fs::read_dir(&installation.0).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains("motrix-pre-native")));
}

#[tokio::test]
async fn native_restart_restores_only_native_tasks_and_reconciles_native_unpersisted_creates() {
    let installation = Installation::legacy().await;
    let state = installation.bootstrap().await;
    let pool = &state.core.database.pool;
    sqlx::query("INSERT INTO download_tasks (id, engine_generation, url, file_name, save_dir, status, created_at, updated_at) VALUES (3, 'native-v2', 'https://fixture.invalid/native.bin', 'native.bin', ?, 'paused', 1, 1)")
        .bind(installation.0.join("downloads").to_str().unwrap()).execute(pool).await.unwrap();
    let operation = TaskOperation::with_id(
        "native-create",
        4,
        TaskOperationType::Create,
        "prepared",
        TaskOperationContext {
            engine_generation: Some(EngineGeneration::NativeV2),
            ..Default::default()
        },
    );
    begin_task_operation(pool, &operation).await.unwrap();
    state.core.database.pool.close().await;

    let restored = installation.bootstrap().await;
    let tasks = restored.core.download_tasks.list().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, 3);
    assert_eq!(tasks[0].engine_generation, EngineGeneration::NativeV2);
    assert_eq!(restored.core.next_task_id.load(Ordering::Relaxed), 5);
    let status: String =
        sqlx::query_scalar("SELECT status FROM task_operations WHERE id = 'native-create'")
            .fetch_one(&restored.core.database.pool)
            .await
            .unwrap();
    assert_eq!(status, "failed");
    assert!(!installation.data().join("aria2-runtime.json").exists());
    restored.core.database.pool.close().await;
}
