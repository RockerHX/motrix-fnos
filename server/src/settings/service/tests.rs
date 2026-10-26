use super::*;
use crate::database::connect_database;

#[tokio::test]
async fn concurrent_settings_and_token_updates_preserve_both_values_after_reopen() {
    let root = std::env::temp_dir().join(format!(
        "motrix-settings-concurrency-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("settings.sqlite");
    let database = connect_database(path.clone()).await.unwrap();
    let directory = root.display().to_string();
    for index in 1..=32 {
        let config = AppConfig {
            default_download_dir: directory.clone(),
            max_concurrent_downloads: index,
            ..AppConfig::default()
        };
        let token = if index % 2 == 0 {
            String::new()
        } else {
            format!("token-{index}")
        };
        let paths = vec![directory.clone()];
        let (saved, rotated) = tokio::join!(
            save_app_config(&database.pool, config.clone(), &directory, &paths, &root),
            save_json_rpc_token(&database.pool, &token),
        );
        assert_eq!(saved.unwrap(), config);
        assert_eq!(rotated.unwrap(), token);
        assert_eq!(
            load_app_config_from_pool(&database.pool, &directory)
                .await
                .unwrap(),
            config
        );
        assert_eq!(load_json_rpc_token(&database.pool).await.unwrap(), token);
    }
    database.pool.close().await;
    let reopened = connect_database(path).await.unwrap();
    assert_eq!(
        load_app_config_from_pool(&reopened.pool, &directory)
            .await
            .unwrap()
            .max_concurrent_downloads,
        32
    );
    assert_eq!(load_json_rpc_token(&reopened.pool).await.unwrap(), "");
    reopened.pool.close().await;
    std::fs::remove_dir_all(root).unwrap();
}

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
                    http_max_connections: 6,
                    connect_timeout: 90,
                    max_tries: 8,
                    download_limit: 1024,
                    upload_limit: 2048,
                    language: "en-US".to_string(),
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
            save_json_rpc_token(&database.pool, "  test-token  ")
                .await
                .expect("token should save");
            save_app_config(
                &database.pool,
                AppConfig {
                    default_download_dir: "/tmp/downloads".to_string(),
                    max_concurrent_downloads: 1,
                    http_max_connections: 6,
                    connect_timeout: 90,
                    max_tries: 8,
                    download_limit: 1024,
                    upload_limit: 2048,
                    language: "en-US".to_string(),
                },
                "/app/data",
                &["/tmp/downloads".to_string()],
                std::path::Path::new("/app/data"),
            )
            .await
            .expect("ordinary settings should preserve token");

            let loaded = load_app_config_from_pool(&database.pool, "/app/data")
                .await
                .expect("config should load");
            assert_eq!(loaded.default_download_dir, "/tmp/downloads");
            assert_eq!(loaded.max_concurrent_downloads, 1);
            assert_eq!(loaded.http_max_connections, 6);
            assert_eq!(loaded.connect_timeout, 90);
            assert_eq!(loaded.max_tries, 8);
            assert_eq!(loaded.download_limit, 1024);
            assert_eq!(loaded.upload_limit, 2048);
            assert_eq!(loaded.language, "en-US");
            assert_eq!(
                load_json_rpc_token(&database.pool)
                    .await
                    .expect("token should load"),
                "test-token"
            );
            assert_eq!(
                load_lan_json_rpc_config(&database.pool)
                    .await
                    .expect("default LAN config should load"),
                LanJsonRpcConfig::default()
            );
            let lan_config = save_lan_json_rpc_config(
                &database.pool,
                &LanJsonRpcConfig {
                    enabled: true,
                    token: "  lan-token  ".to_string(),
                    allow_shared_address_space: true,
                },
            )
            .await
            .expect("LAN config should save");
            assert_eq!(
                lan_config,
                LanJsonRpcConfig {
                    enabled: true,
                    token: "lan-token".to_string(),
                    allow_shared_address_space: true,
                }
            );
            assert_eq!(
                load_lan_json_rpc_config(&database.pool)
                    .await
                    .expect("LAN config should reload"),
                lan_config
            );

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
                    ..AppConfig::default()
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
                VALUES ('download', '{"defaultDownloadDir":"/tmp/downloads","maxConcurrentDownloads":128,"split":8,"maxConnectionPerServer":3,"minSplitSize":"5M","downloadLimit":0,"uploadLimit":0,"autoStartEnabled":true,"notificationsEnabled":true}', 1)
                "#,
            )
            .execute(&database.pool)
            .await
            .expect("legacy config should insert");

            let loaded = load_app_config_from_pool(&database.pool, "/app/data")
                .await
                .expect("legacy config should load");

            assert_eq!(loaded.default_download_dir, "/tmp/downloads");
            assert_eq!(loaded.max_concurrent_downloads, 128);
            assert_eq!(loaded.http_max_connections, 3);
            assert_eq!(loaded.connect_timeout, DEFAULT_CONNECT_TIMEOUT);
            assert_eq!(loaded.max_tries, DEFAULT_MAX_TRIES);
            assert_eq!(loaded.language, "zh-CN");
            assert_eq!(
                load_json_rpc_token(&database.pool)
                    .await
                    .expect("token should load"),
                ""
            );
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
            ..AppConfig::default()
        },
        "/app/data",
    )
    .expect("config should normalize");

    assert_eq!(config.language, "zh-CN");
}

#[test]
fn app_config_normalizes_download_tuning_bounds() {
    let lower = normalize_app_config(
        AppConfig {
            default_download_dir: "/tmp/downloads".to_string(),
            max_concurrent_downloads: 0,
            http_max_connections: 1,
            connect_timeout: 0,
            max_tries: 0,
            download_limit: 0,
            upload_limit: 0,
            language: "zh-CN".to_string(),
        },
        "/app/data",
    )
    .expect("config should normalize");
    assert_eq!(lower.max_concurrent_downloads, 1);
    assert_eq!(lower.http_max_connections, DEFAULT_HTTP_MAX_CONNECTIONS);
    assert_eq!(lower.connect_timeout, 1);
    assert_eq!(lower.max_tries, 1);

    let upper = normalize_app_config(
        AppConfig {
            default_download_dir: "/tmp/downloads".to_string(),
            max_concurrent_downloads: MAX_CONCURRENT_DOWNLOADS_LIMIT + 1,
            http_max_connections: HTTP_MAX_CONNECTIONS_LIMIT,
            connect_timeout: MAX_CONNECT_TIMEOUT_LIMIT + 1,
            max_tries: MAX_TRIES_LIMIT + 1,
            download_limit: 0,
            upload_limit: 0,
            language: "zh-CN".to_string(),
        },
        "/app/data",
    )
    .expect("config should normalize");
    assert_eq!(
        upper.max_concurrent_downloads,
        MAX_CONCURRENT_DOWNLOADS_LIMIT
    );
    assert_eq!(upper.http_max_connections, HTTP_MAX_CONNECTIONS_LIMIT);
    assert_eq!(upper.connect_timeout, MAX_CONNECT_TIMEOUT_LIMIT);
    assert_eq!(upper.max_tries, MAX_TRIES_LIMIT);
}

#[test]
fn legacy_connection_settings_use_the_lower_valid_limit() {
    for (value, expected) in [
        (serde_json::json!({}), 1),
        (
            serde_json::json!({"split": 12, "maxConnectionPerServer": 8}),
            8,
        ),
        (
            serde_json::json!({"split": -1, "maxConnectionPerServer": 8}),
            5,
        ),
        (
            serde_json::json!({"split": "invalid", "maxConnectionPerServer": 0}),
            1,
        ),
    ] {
        let mut value = value;
        migrate_connection_settings(&mut value).expect("legacy settings should migrate");
        assert_eq!(value["httpMaxConnections"], expected);
        assert!(value.get("split").is_none());
        assert!(value.get("maxConnectionPerServer").is_none());
    }
}

#[test]
fn invalid_stored_connection_limit_defaults_but_api_input_is_rejected() {
    for value in [
        serde_json::json!(-1),
        serde_json::json!("bad"),
        serde_json::json!(65),
    ] {
        let mut stored = serde_json::json!({"httpMaxConnections": value});
        migrate_connection_settings(&mut stored).expect("stored config should migrate");
        assert_eq!(stored["httpMaxConnections"], DEFAULT_HTTP_MAX_CONNECTIONS);
    }

    for value in [
        serde_json::json!(-1),
        serde_json::json!("bad"),
        serde_json::json!(65),
    ] {
        let error = parse_app_config_request(serde_json::json!({
            "defaultDownloadDir": "/downloads",
            "maxConcurrentDownloads": 5,
            "httpMaxConnections": value,
            "downloadLimit": 0,
            "uploadLimit": 0
        }))
        .expect_err("invalid API value should fail");
        assert!(error.starts_with("settings_connections_invalid"));
    }

    let error = parse_app_config_request(serde_json::json!({"minSplitSize": "5M"}))
        .expect_err("deprecated option should fail");
    assert!(error.starts_with("settings_option_unsupported"));
}
