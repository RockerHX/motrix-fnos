use super::*;
use crate::database::connect_database;

#[test]
fn app_config_uses_defaults_and_round_trips_saved_values() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-app-config-test-{}.sqlite",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system time should be valid")
                    .as_millis()
            ));
            let database = connect_database(path.clone())
                .await
                .expect("database should connect");

            let default_config = load_app_config_from_pool(&database.pool, "/app/data")
                .await
                .expect("default config should load");
            assert_eq!(default_config.default_download_dir, "/app/data");

            let saved = normalize_app_config(
                AppConfig {
                    default_download_dir: "/tmp/downloads".to_string(),
                    max_concurrent_downloads: 0,
                    download_limit: 1024,
                    upload_limit: 2048,
                    language: "en-US".to_string(),
                    json_rpc_token: "  test-token  ".to_string(),
                },
                "/app/data",
            )
            .expect("config should normalize");
            save_app_config(
                &database.pool,
                saved,
                "/app/data",
                &["/tmp/downloads".to_string()],
                std::path::Path::new("/app/data"),
            )
            .await
            .expect("config should save");

            let loaded = load_app_config_from_pool(&database.pool, "/app/data")
                .await
                .expect("config should load");
            assert_eq!(loaded.default_download_dir, "/tmp/downloads");
            assert_eq!(loaded.max_concurrent_downloads, 1);
            assert_eq!(loaded.download_limit, 1024);
            assert_eq!(loaded.upload_limit, 2048);
            assert_eq!(loaded.language, "en-US");
            assert_eq!(loaded.json_rpc_token, "test-token");

            database.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
}

#[test]
fn app_config_rejects_unauthorized_default_download_dir() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-unauthorized-app-config-test-{}.sqlite",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system time should be valid")
                    .as_nanos()
            ));
            let database = connect_database(path.clone())
                .await
                .expect("database should connect");

            let error = save_app_config(
                &database.pool,
                AppConfig {
                    default_download_dir: "/tmp/downloads".to_string(),
                    max_concurrent_downloads: 5,
                    download_limit: 0,
                    upload_limit: 0,
                    language: "zh-CN".to_string(),
                    json_rpc_token: String::new(),
                },
                "/app/data",
                &["/app/data".to_string()],
                std::path::Path::new("/app/data"),
            )
            .await
            .expect_err("unauthorized directory should fail");

            assert_eq!(error, "默认下载目录不在已授权目录列表中");

            database.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
}

#[test]
fn app_config_accepts_legacy_saved_values() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-legacy-app-config-test-{}.sqlite",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system time should be valid")
                    .as_millis()
            ));
            let database = connect_database(path.clone())
                .await
                .expect("database should connect");

            sqlx::query(
                r#"
                INSERT INTO app_config (key, value, updated_at)
                VALUES ('download', '{"defaultDownloadDir":"/tmp/downloads","maxConcurrentDownloads":128,"downloadLimit":0,"uploadLimit":0,"autoStartEnabled":true,"notificationsEnabled":true}', 1)
                "#,
            )
            .execute(&database.pool)
            .await
            .expect("legacy config should insert");

            let loaded = load_app_config_from_pool(&database.pool, "/app/data")
                .await
                .expect("legacy config should load");

            assert_eq!(loaded.default_download_dir, "/tmp/downloads");
            assert_eq!(loaded.max_concurrent_downloads, 64);
            assert_eq!(loaded.language, "zh-CN");
            assert_eq!(loaded.json_rpc_token, "");
            let serialized = serde_json::to_value(&loaded).expect("config should serialize");
            assert!(serialized.get("autoStartEnabled").is_none());
            assert!(serialized.get("notificationsEnabled").is_none());

            database.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
}

#[test]
fn app_config_falls_back_to_default_language_for_invalid_values() {
    let config = normalize_app_config(
        AppConfig {
            default_download_dir: "/tmp/downloads".to_string(),
            max_concurrent_downloads: 5,
            download_limit: 0,
            upload_limit: 0,
            language: "fr-FR".to_string(),
            json_rpc_token: String::new(),
        },
        "/app/data",
    )
    .expect("config should normalize");

    assert_eq!(config.language, "zh-CN");
}
