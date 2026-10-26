use super::*;
use crate::settings::service::apply_runtime_download_config;

use crate::app::{
    bootstrap_http_app_state, ServerRuntimeConfig, DEFAULT_HTTP_ADDR, DEFAULT_JSONRPC_ADDR,
};
use crate::config::aria2::{Aria2BinarySource, EngineGeneration};
use crate::database::settings::{get_download_proxy_config, BtRuntimeApplyStatus};
use crate::runtime::Aria2LifecyclePhase;
use crate::tasks::{DownloadTask, DownloadTaskStatus, TaskProxyBinding};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

static LAN_TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

#[tokio::test]
async fn concurrent_public_settings_updates_publish_in_database_order_and_keep_failures_private() {
    use crate::settings::service::{load_app_config_from_pool, load_json_rpc_token};
    let (state, runtime) = lan_test_state("concurrent-public-settings").await;
    state
        .aria2_lifecycle
        .set_phase(Aria2LifecyclePhase::Stopping)
        .unwrap();
    let directory = runtime.app_data_dir.display().to_string();
    let config = AppConfig {
        default_download_dir: directory.clone(),
        max_concurrent_downloads: 19,
        ..AppConfig::default()
    };
    let guard = state.settings_update_lock.lock().await;
    let first_state = state.clone();
    let first =
        tokio::spawn(async move { update_public_rpc_token(&first_state, "first-token").await });
    tokio::task::yield_now().await;
    let second_state = state.clone();
    let second = tokio::spawn(async move { update_public_rpc_token(&second_state, "").await });
    tokio::task::yield_now().await;
    assert!(!first.is_finished() && !second.is_finished());
    drop(guard);
    first.await.unwrap().unwrap();
    second.await.unwrap().unwrap();
    assert_eq!(state.json_rpc_token(), "");
    let paths = vec![directory.clone()];
    let (saved, token) = tokio::join!(
        update_app_config(&state, config.clone(), &directory, &paths),
        update_public_rpc_token(&state, "new-token"),
    );
    assert_eq!(
        saved.unwrap(),
        (config.clone(), RuntimeApplyStatus::Deferred)
    );
    token.unwrap();
    assert_eq!(state.current_app_config().await, config);
    assert_eq!(state.json_rpc_token(), "new-token");
    assert_eq!(
        load_app_config_from_pool(&state.core.database.pool, &directory)
            .await
            .unwrap(),
        config
    );
    assert_eq!(
        load_json_rpc_token(&state.core.database.pool)
            .await
            .unwrap(),
        "new-token"
    );
    assert!(state.aria2_runtime_snapshot().is_none());
    for (token, accepted) in [("new-token", true), ("first-token", false)] {
        use tower::ServiceExt;
        let response = crate::api::jsonrpc_router(state.clone()).oneshot(
            axum::http::Request::builder().method("POST").uri("/jsonrpc")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(json!({"jsonrpc":"2.0", "id":1, "method":"aria2.getGlobalOption", "params":[format!("token:{token}")]}).to_string())).unwrap()
        ).await.unwrap();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let response: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(response.get("result").is_some(), accepted, "{response}");
    }
    assert!(get_settings(State(state.clone())).await.unwrap().0 == config);
    state.core.database.pool.close().await;
    assert!(update_public_rpc_token(&state, "must-not-publish")
        .await
        .is_err());
    assert!(update_app_config(
        &state,
        AppConfig {
            max_concurrent_downloads: 20,
            ..config.clone()
        },
        &directory,
        &paths
    )
    .await
    .is_err());
    assert_eq!(state.json_rpc_token(), "new-token");
    assert_eq!(state.current_app_config().await, config);
    std::fs::remove_dir_all(runtime.app_data_dir).unwrap();
}

#[test]
fn token_status_masks_long_and_short_tokens_without_returning_raw_values() {
    assert_eq!(
        json_rpc_token_status(""),
        JsonRpcTokenStatus {
            configured: false,
            masked_token: None,
        }
    );
    assert_eq!(
        json_rpc_token_status("short"),
        JsonRpcTokenStatus {
            configured: true,
            masked_token: Some("••••••••".to_string()),
        }
    );
    assert_eq!(
        json_rpc_token_status("long-token-a1b2"),
        JsonRpcTokenStatus {
            configured: true,
            masked_token: Some("••••••••a1b2".to_string()),
        }
    );
}

#[test]
fn public_app_config_never_serializes_legacy_token_field() {
    let config = AppConfig {
        default_download_dir: "/downloads".to_string(),
        max_concurrent_downloads: 5,
        download_limit: 0,
        upload_limit: 0,
        language: "zh-CN".to_string(),
        ..AppConfig::default()
    };
    let value = serde_json::to_value(config).expect("config should serialize");
    assert!(value.get("jsonRpcToken").is_none());
}

#[test]
fn update_settings_response_flattens_config_and_uses_optional_runtime_status() {
    let config = AppConfig {
        default_download_dir: "/downloads".to_string(),
        max_concurrent_downloads: 128,
        download_limit: 0,
        upload_limit: 0,
        language: "zh-CN".to_string(),
        ..AppConfig::default()
    };
    let response = UpdateSettingsResponse {
        config: config.clone(),
        runtime_apply: Some(RuntimeApplyStatus::Applied),
    };
    let value = serde_json::to_value(response).expect("response should serialize");

    assert_eq!(value["maxConcurrentDownloads"], 128);
    assert_eq!(value["runtimeApply"], "applied");

    let response_without_status = UpdateSettingsResponse {
        config,
        runtime_apply: None,
    };
    let value = serde_json::to_value(response_without_status)
        .expect("response without status should serialize");
    assert!(value.get("runtimeApply").is_none());
}

#[tokio::test]
async fn runtime_apply_reports_deferred_when_lifecycle_is_transitioning() {
    let (state, runtime) = lan_test_state("runtime-deferred").await;
    state
        .aria2_lifecycle
        .set_phase(Aria2LifecyclePhase::Stopping)
        .expect("lifecycle should enter stopping");

    let status = apply_runtime_download_config(&state, &test_app_config()).await;

    assert_eq!(status, RuntimeApplyStatus::Deferred);
    cleanup_test_state(&state, runtime).await;
}

#[tokio::test]
async fn runtime_apply_reports_applied_when_rpc_accepts_options() {
    let (port, server) = spawn_settings_rpc(false).await;
    let (mut state, runtime) = lan_test_state("runtime-applied").await;
    configure_rpc_runtime(&mut state, port);

    let status = apply_runtime_download_config(&state, &test_app_config()).await;

    assert_eq!(status, RuntimeApplyStatus::Applied);
    server.abort();
    cleanup_test_state(&state, runtime).await;
}

#[tokio::test]
async fn runtime_apply_reports_failed_when_rpc_rejects_options() {
    let (port, server) = spawn_settings_rpc(true).await;
    let (mut state, runtime) = lan_test_state("runtime-failed").await;
    configure_rpc_runtime(&mut state, port);

    let status = apply_runtime_download_config(&state, &test_app_config()).await;

    assert_eq!(status, RuntimeApplyStatus::Failed);
    let directory = runtime.app_data_dir.display().to_string();
    let config = AppConfig {
        default_download_dir: directory.clone(),
        ..test_app_config()
    };
    let result = update_app_config(&state, config.clone(), &directory, &[directory.clone()])
        .await
        .unwrap();
    assert_eq!(result, (config.clone(), RuntimeApplyStatus::Failed));
    assert_eq!(state.current_app_config().await, config);
    assert_eq!(
        crate::settings::service::load_app_config_from_pool(&state.core.database.pool, &directory)
            .await
            .unwrap(),
        config
    );
    server.abort();
    cleanup_test_state(&state, runtime).await;
}

#[tokio::test]
async fn legacy_settings_payload_uses_tuning_defaults_and_updates_runtime_cache() {
    let (state, runtime) = lan_test_state("legacy-settings-payload").await;
    let payload = json!({
        "defaultDownloadDir": runtime.app_data_dir.display().to_string(),
        "maxConcurrentDownloads": 8,
        "downloadLimit": 1024,
        "uploadLimit": 2048,
        "language": "zh-CN",
    });

    let response = update_settings(State(state.clone()), ApiJson(payload))
        .await
        .expect("legacy payload should save")
        .0;
    assert_eq!(response.runtime_apply, Some(RuntimeApplyStatus::Deferred));
    assert_eq!(response.config.http_max_connections, 1);
    assert_eq!(response.config.connect_timeout, 60);
    assert_eq!(response.config.max_tries, 5);
    assert_eq!(state.current_app_config().await, response.config);

    cleanup_test_state(&state, runtime).await;
}

#[tokio::test]
async fn proxy_settings_handlers_round_trip_masked_status_and_clear_safely() {
    let (state, runtime) = lan_test_state("proxy-round-trip").await;
    let saved = put_download_proxy(
        State(state.clone()),
        ApiJson(serde_json::json!({
            "proxyUrl": "http://ApiUser:ApiPassword@Proxy.Example:7890"
        })),
    )
    .await
    .expect("proxy should save")
    .0;
    assert!(saved.status.configured);
    assert_eq!(saved.status.revision, 1);
    let masked = saved
        .status
        .masked_proxy_url
        .as_deref()
        .expect("masked proxy should exist");
    assert!(masked.contains("***:***@proxy.example:7890"));
    assert!(!masked.contains("ApiUser"));
    assert!(!masked.contains("ApiPassword"));

    let loaded = get_download_proxy(State(state.clone()))
        .await
        .expect("proxy status should load")
        .0;
    assert_eq!(loaded, saved.status);
    assert_eq!(
        clear_download_proxy(State(state.clone()))
            .await
            .expect("unused proxy should clear"),
        StatusCode::NO_CONTENT
    );
    assert!(
        !get_download_proxy(State(state.clone()))
            .await
            .expect("cleared proxy status should load")
            .0
            .configured
    );

    state.core.database.pool.close().await;
    let _ = std::fs::remove_dir_all(runtime.app_data_dir);
}

#[tokio::test]
async fn bt_proxy_update_persists_global_mode_and_defers_until_engine_start() {
    let (state, runtime) = lan_test_state("bt-proxy-deferred").await;
    let saved = put_download_proxy(
        State(state.clone()),
        ApiJson(serde_json::json!({
            "proxyUrl": "socks4://proxy.example:1080",
            "btUseProxy": true
        })),
    )
    .await
    .expect("BT proxy should save while Aria2 is stopped")
    .0;

    assert!(saved.status.bt_use_proxy);
    assert_eq!(
        saved.status.bt_runtime_apply,
        crate::database::settings::BtRuntimeApplyStatus::Deferred
    );
    assert_eq!(saved.status.revision, 1);
    assert!(saved.applied_task_ids.is_empty());
    assert!(saved.deferred_task_ids.is_empty());

    cleanup_test_state(&state, runtime).await;
}

#[tokio::test]
async fn bt_proxy_switch_verifies_before_resume_and_keeps_failures_paused() {
    use BtRuntimeApplyStatus::{Applied, Blocked, Deferred};
    for (scenario, expected_apply) in [
        ("success", Applied),
        ("write-rejected", Blocked),
        ("write-unknown", Blocked),
        ("verify-mismatch", Blocked),
        ("verify-unknown", Blocked),
        ("save-failed", Applied),
        ("status-save-failed", Deferred),
        ("disable-rejected", Blocked),
    ] {
        let (mut state, runtime) = lan_test_state(scenario).await;
        let pool = state.core.database.pool.clone();
        crate::database::settings::replace_download_proxy_config(
            &pool,
            Some("http://SensitiveUser:SensitivePassword@old.example:7890/".to_string()),
            Some(scenario == "disable-rejected"),
            Applied,
            1,
        )
        .await
        .unwrap();
        std::fs::write(
            &runtime.accessible_paths_path,
            json!({"paths": [runtime.app_data_dir]}).to_string(),
        )
        .unwrap();
        for (id, source, status) in [
            (1, "torrent", "active"),
            (2, "magnet", "pending"),
            (3, "torrent", "paused"),
            (4, "url", "active"),
        ] {
            let mut task: DownloadTask = serde_json::from_value(json!({
                "id": id, "engineGeneration": EngineGeneration::NativeV2,
                "url": if source == "magnet" { "magnet:?xt=urn:btih:test" } else { "https://example.com/test" },
                "sourceType": source, "status": status, "gid": format!("gid-{id}"),
                "fileName": "test", "saveDir": runtime.app_data_dir.join(format!("task-{id}")),
                "category": "默认", "totalLength": 100, "completedLength": 10,
                "downloadSpeed": 0, "confirmationRequired": false, "files": [],
                "createdAt": 1, "updatedAt": 1
            })).unwrap();
            if source == "url" {
                task.use_proxy = true;
                task.proxy_binding =
                    TaskProxyBinding::override_url("http://private.example:7890/".to_string());
            } else {
                task.owned_task_dir = Some(task.save_dir.clone());
            }
            crate::database::tasks::upsert_download_task(&pool, &task)
                .await
                .unwrap();
            state
                .core
                .download_tasks
                .with_tasks_mut(|tasks| tasks.push(task))
                .unwrap();
        }
        let http_task = state.core.download_tasks.list().unwrap().pop().unwrap();
        let requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let recorded = Arc::clone(&requests);
        let rpc_pool = pool.clone();
        let engine = Arc::new(std::sync::Mutex::new((
            String::new(),
            std::collections::HashSet::<String>::new(),
        )));
        let app = Router::new().route("/jsonrpc", post(move |Json(payload): Json<Value>| {
            let (pool, recorded, engine) = (rpc_pool.clone(), Arc::clone(&recorded), Arc::clone(&engine));
            async move {
                let method = payload["method"].as_str().unwrap();
                let stored = get_download_proxy_config(&pool).await.unwrap().unwrap();
                if method == "aria2.changeGlobalOption" {
                    let statuses = sqlx::query_scalar::<_, String>("SELECT status FROM download_tasks WHERE source_type != 'url'").fetch_all(&pool).await.unwrap();
                    assert_eq!(statuses, ["paused", "paused", "paused"]);
                    assert_eq!(stored.bt_runtime_apply, BtRuntimeApplyStatus::Deferred);
                }
                recorded.lock().unwrap().push(method.to_string());
                let gid = payload["params"][1].as_str().unwrap_or_default();
                let mut engine = engine.lock().unwrap();
                let result = match method {
                    "aria2.pause" => { engine.1.insert(gid.to_string()); json!(gid) }
                    "aria2.unpause" => {
                        assert_eq!(stored.bt_runtime_apply, BtRuntimeApplyStatus::Applied);
                        assert_ne!(gid, "gid-3");
                        engine.1.remove(gid);
                        json!(gid)
                    }
                    "aria2.tellStatus" => json!({"gid": gid, "status": if engine.1.contains(gid) {"paused"} else {"active"}, "totalLength": "100", "completedLength": "10", "downloadSpeed": "0"}),
                    "aria2.changeGlobalOption" => {
                        let target = payload["params"][1]["bt-proxy"].as_str().unwrap();
                        assert_eq!(target, if stored.bt_use_proxy { stored.proxy_url.as_deref().unwrap() } else { "" });
                        if matches!(scenario, "write-rejected" | "disable-rejected") {
                            return Json(json!({"error": {"message": format!("invalid proxy {target}")}})).into_response();
                        }
                        engine.0 = target.to_string();
                        if scenario == "write-unknown" { return StatusCode::SERVICE_UNAVAILABLE.into_response(); }
                        json!("OK")
                    }
                    "aria2.getGlobalOption" => {
                        if scenario == "verify-unknown" { return StatusCode::SERVICE_UNAVAILABLE.into_response(); }
                        json!({"bt-proxy": if scenario == "verify-mismatch" { "http://wrong.example/" } else { &engine.0 }})
                    }
                    other => panic!("unexpected RPC: {other}"),
                };
                Json(json!({"result": result})).into_response()
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        Arc::get_mut(&mut state)
            .unwrap()
            .base_aria2_config
            .state_dir = Some(runtime.app_data_dir.join("state").display().to_string());
        configure_rpc_runtime(&mut state, listener.local_addr().unwrap().port());
        state
            .aria2_lifecycle
            .set_phase(Aria2LifecyclePhase::Ready)
            .unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        if scenario.contains("save-failed") {
            let condition = if scenario == "status-save-failed" {
                "AND json_extract(NEW.value, '$.btRuntimeApply') = 'applied'"
            } else {
                ""
            };
            sqlx::query(&format!("CREATE TRIGGER reject_proxy_save BEFORE UPDATE ON app_config WHEN NEW.key = 'download_proxy' {condition} BEGIN SELECT RAISE(ABORT, 'injected save failure'); END"))
                .execute(&pool).await.unwrap();
        }
        let updates = if scenario == "success" {
            vec![
                json!({"btUseProxy": true}),
                json!({"proxyUrl": "socks5://new.example:1080"}),
                json!({"btUseProxy": false}),
            ]
        } else {
            vec![json!({"btUseProxy": scenario != "disable-rejected"})]
        };
        for update in updates {
            requests.lock().unwrap().clear();
            let result = put_download_proxy(State(state.clone()), ApiJson(update)).await;
            let saved = get_download_proxy_config(&pool).await.unwrap().unwrap();
            assert_eq!(saved.bt_runtime_apply, expected_apply, "{scenario}");
            let calls = requests.lock().unwrap().clone();
            let tasks = crate::database::tasks::list_download_tasks(&pool)
                .await
                .unwrap();
            assert_eq!(
                state.core.download_tasks.list().unwrap().pop().unwrap(),
                http_task
            );
            assert!(tasks.iter().filter(|t| t.id < 4).all(|t| {
                t.status
                    == if scenario == "success" && t.id < 3 {
                        DownloadTaskStatus::Active
                    } else {
                        DownloadTaskStatus::Paused
                    }
            }));
            if scenario.contains("save-failed") {
                assert_eq!(
                    result.unwrap_err().status(),
                    StatusCode::INTERNAL_SERVER_ERROR
                );
                assert_eq!(saved.bt_use_proxy, scenario == "status-save-failed");
            } else {
                let response = result.unwrap().0;
                assert_eq!(response.status.bt_runtime_apply, expected_apply);
                let public = serde_json::to_string(&response).unwrap();
                for secret in ["SensitiveUser", "SensitivePassword"] {
                    assert!(!public.contains(secret));
                    assert!(state
                        .core
                        .debug_logs
                        .list()
                        .iter()
                        .all(|entry| !entry.message.contains(secret)));
                }
            }
            if scenario == "success" {
                let changes = calls
                    .iter()
                    .filter(|c| *c != "aria2.tellStatus")
                    .map(String::as_str)
                    .collect::<Vec<_>>();
                assert_eq!(
                    changes,
                    [
                        "aria2.pause",
                        "aria2.pause",
                        "aria2.changeGlobalOption",
                        "aria2.getGlobalOption",
                        "aria2.unpause",
                        "aria2.unpause"
                    ]
                );
            } else {
                assert!(!calls.iter().any(|c| c == "aria2.unpause"));
                assert_eq!(
                    calls
                        .iter()
                        .filter(|c| *c == "aria2.changeGlobalOption")
                        .count(),
                    usize::from(scenario != "save-failed")
                );
            }
        }
        if scenario == "disable-rejected" {
            requests.lock().unwrap().clear();
            let saved = put_download_proxy(
                State(state.clone()),
                ApiJson(json!({"proxyUrl": "http://http-only.example:7890"})),
            )
            .await
            .unwrap()
            .0;
            assert_eq!(saved.status.bt_runtime_apply, Blocked);
            assert!(requests.lock().unwrap().is_empty());
        }
        server.abort();
        cleanup_test_state(&state, runtime).await;
    }
}

#[tokio::test]
async fn enabling_bt_proxy_rejects_https_without_changing_configuration() {
    let (state, runtime) = lan_test_state("bt-proxy-https").await;
    let _ = put_download_proxy(
        State(state.clone()),
        ApiJson(serde_json::json!({
            "proxyUrl": "https://proxy.example:8443"
        })),
    )
    .await
    .expect("HTTPS proxy should remain valid for HTTP tasks");

    let error = put_download_proxy(
        State(state.clone()),
        ApiJson(serde_json::json!({ "btUseProxy": true })),
    )
    .await
    .expect_err("BT should reject HTTPS proxy protocol");
    assert_eq!(error.status(), StatusCode::BAD_REQUEST);
    let body = error
        .into_response()
        .into_body()
        .collect()
        .await
        .expect("error body should collect")
        .to_bytes();
    let body = String::from_utf8(body.to_vec()).expect("error body should be utf-8");
    assert!(body.contains("bt_proxy_protocol_unsupported"));

    let loaded = get_download_proxy(State(state.clone()))
        .await
        .expect("proxy status should load")
        .0;
    assert!(!loaded.bt_use_proxy);
    assert_eq!(loaded.revision, 1);
    cleanup_test_state(&state, runtime).await;
}

#[tokio::test]
async fn invalid_proxy_api_error_never_echoes_credentials() {
    let (state, runtime) = lan_test_state("proxy-invalid").await;
    let raw = "http://SecretUser:SecretPassword@proxy.example:7890?token=private";
    let error = put_download_proxy(
        State(state.clone()),
        ApiJson(serde_json::json!({ "proxyUrl": raw })),
    )
    .await
    .expect_err("query should be rejected");
    assert_eq!(error.status(), StatusCode::BAD_REQUEST);
    let response = error.into_response();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("error body should collect")
        .to_bytes();
    let body = String::from_utf8(body.to_vec()).expect("error body should be utf-8");
    assert!(body.contains("proxy_invalid_url"));
    assert!(!body.contains("SecretUser"));
    assert!(!body.contains("SecretPassword"));
    assert!(!body.contains("token=private"));

    state.core.database.pool.close().await;
    let _ = std::fs::remove_dir_all(runtime.app_data_dir);
}

#[tokio::test]
async fn lan_json_rpc_first_enable_preserves_token_and_rotation_invalidates_old_value() {
    let (state, runtime) = lan_test_state("lifecycle").await;

    let enabled = update_lan_json_rpc(
        State(state.clone()),
        ApiJson(UpdateLanJsonRpcRequest {
            enabled: true,
            allow_shared_address_space: Some(true),
        }),
    )
    .await
    .expect("first enable should succeed")
    .0;
    let first_token = enabled
        .issued_token
        .clone()
        .expect("first enable should issue a token");
    assert_eq!(URL_SAFE_NO_PAD.decode(&first_token).unwrap().len(), 32);
    assert!(enabled.status.enabled);
    assert!(enabled.status.configured);
    assert!(enabled.status.allow_shared_address_space);
    assert_eq!(enabled.status.port, 17082);
    assert!(!enabled
        .status
        .masked_token
        .as_deref()
        .unwrap_or_default()
        .contains(&first_token));

    let disabled = update_lan_json_rpc(
        State(state.clone()),
        ApiJson(UpdateLanJsonRpcRequest {
            enabled: false,
            allow_shared_address_space: None,
        }),
    )
    .await
    .expect("disable should succeed")
    .0;
    assert!(!disabled.status.enabled);
    assert!(disabled.status.allow_shared_address_space);
    assert!(disabled.issued_token.is_none());

    let reenabled = update_lan_json_rpc(
        State(state.clone()),
        ApiJson(UpdateLanJsonRpcRequest {
            enabled: true,
            allow_shared_address_space: None,
        }),
    )
    .await
    .expect("re-enable should succeed")
    .0;
    assert!(reenabled.issued_token.is_none());
    assert!(reenabled.status.allow_shared_address_space);
    assert_eq!(state.lan_json_rpc_config().await.token, first_token);

    let rotated = rotate_lan_json_rpc_token(State(state.clone()))
        .await
        .expect("rotation should succeed")
        .0;
    let second_token = rotated.issued_token.expect("rotation should issue a token");
    assert_ne!(second_token, first_token);
    assert_eq!(state.lan_json_rpc_config().await.token, second_token);

    state.core.database.pool.close().await;
    let restored = bootstrap_http_app_state(&runtime)
        .await
        .expect("LAN config should restore after restart");
    let restored_config = restored.lan_json_rpc_config().await;
    assert!(restored_config.enabled);
    assert!(restored_config.allow_shared_address_space);
    assert_eq!(restored_config.token, second_token);
    restored.core.database.pool.close().await;
    let _ = std::fs::remove_dir_all(runtime.app_data_dir);
}

#[tokio::test]
async fn lan_json_rpc_storage_failure_does_not_change_memory_state() {
    let (state, runtime) = lan_test_state("storage-failure").await;
    state.core.database.pool.close().await;

    let error = update_lan_json_rpc(
        State(state.clone()),
        ApiJson(UpdateLanJsonRpcRequest {
            enabled: true,
            allow_shared_address_space: Some(true),
        }),
    )
    .await
    .expect_err("closed database should reject update");

    assert_eq!(
        error.status(),
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        state.lan_json_rpc_config().await,
        LanJsonRpcConfig::default()
    );
    let _ = std::fs::remove_dir_all(runtime.app_data_dir);
}

async fn lan_test_state(label: &str) -> (Arc<HttpAppState>, ServerRuntimeConfig) {
    let index = LAN_TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let app_data_dir = std::env::temp_dir().join(format!(
        "motrix-fnos-lan-settings-{label}-{}-{index}",
        std::process::id()
    ));
    let runtime = ServerRuntimeConfig {
        database_path: app_data_dir.join("motrix-fnos.sqlite"),
        accessible_paths_path: app_data_dir.join("accessible-paths.json"),
        aria2_path: Some(crate::test_support::native_version_only_engine(
            &app_data_dir,
        )),
        app_data_dir,
        http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
        jsonrpc_addr: DEFAULT_JSONRPC_ADDR.parse().expect("addr should parse"),
        lan_jsonrpc_addr: "127.0.0.1:0".parse().expect("addr should parse"),
        trusted_proxy_ips: Vec::new(),
    };
    let state = bootstrap_http_app_state(&runtime)
        .await
        .expect("state should bootstrap");
    (state, runtime)
}

fn test_app_config() -> AppConfig {
    AppConfig {
        default_download_dir: "/downloads".to_string(),
        max_concurrent_downloads: 128,
        http_max_connections: 6,
        connect_timeout: 90,
        max_tries: 8,
        download_limit: 0,
        upload_limit: 0,
        language: "zh-CN".to_string(),
    }
}

fn configure_rpc_runtime(state: &mut Arc<HttpAppState>, port: u16) {
    let mutable_state = Arc::get_mut(state).expect("state should be uniquely owned");
    mutable_state.base_aria2_config.rpc_port = port;
    let config = crate::aria2::runtime_config(
        &mutable_state.base_aria2_config,
        port,
        "settings-test-secret".to_string(),
    );
    state
        .set_aria2_runtime(state.build_aria2_runtime_info(
            42,
            &config,
            Aria2BinarySource::Sidecar,
            Vec::new(),
        ))
        .expect("runtime should save");
}

async fn spawn_settings_rpc(reject_options: bool) -> (u16, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock listener should bind");
    let port = listener
        .local_addr()
        .expect("mock address should exist")
        .port();
    let server = tokio::spawn(async move {
        let app = Router::new().route(
            "/jsonrpc",
            post(move |Json(payload): Json<Value>| async move {
                let method = payload["method"].as_str().unwrap_or_default();
                if method == "aria2.getVersion" {
                    return Json(json!({ "result": { "version": "2.5.5" } }));
                }
                if reject_options && method == "aria2.changeGlobalOption" {
                    return Json(json!({
                        "error": { "code": 1, "message": "options rejected" }
                    }));
                }
                Json(json!({ "result": "OK" }))
            }),
        );
        let _ = axum::serve(listener, app).await;
    });
    (port, server)
}

async fn cleanup_test_state(state: &Arc<HttpAppState>, runtime: ServerRuntimeConfig) {
    state.core.database.pool.close().await;
    let _ = std::fs::remove_dir_all(runtime.app_data_dir);
}

#[tokio::test]
async fn native_proxy_clear_ignores_hidden_legacy_but_keeps_completed_and_removed_references() {
    use crate::database::tasks::upsert_download_task;
    let (mut state, runtime) = lan_test_state("native-clear-reference").await;
    Arc::get_mut(&mut state)
        .unwrap()
        .base_aria2_config
        .state_dir = Some(runtime.app_data_dir.join("state").display().to_string());
    let pool = &state.core.database.pool;
    let mut task: DownloadTask = serde_json::from_value(json!({
        "id": 1, "engineGeneration": "legacy-v1", "url": "https://example.com/file", "sourceType": "url",
        "status": "active", "gid": "old-gid", "fileName": "file", "saveDir": runtime.app_data_dir,
        "category": "默认", "totalLength": 100, "completedLength": 10, "downloadSpeed": 0,
        "confirmationRequired": false, "files": [], "createdAt": 1, "updatedAt": 1
    })).unwrap();
    task.use_proxy = true;
    task.proxy_binding = TaskProxyBinding::profile(None);
    upsert_download_task(pool, &task).await.unwrap();
    crate::database::settings::replace_download_proxy_config(
        pool,
        Some("http://proxy.example:7890/".into()),
        Some(false),
        BtRuntimeApplyStatus::Deferred,
        1,
    )
    .await
    .unwrap();
    assert_eq!(
        clear_download_proxy(State(state.clone())).await.unwrap(),
        StatusCode::NO_CONTENT
    );
    for status in [DownloadTaskStatus::Complete, DownloadTaskStatus::Removed] {
        crate::database::settings::replace_download_proxy_config(
            pool,
            Some("http://proxy.example:7890/".into()),
            None,
            BtRuntimeApplyStatus::Deferred,
            2,
        )
        .await
        .unwrap();
        task.id = 2;
        task.engine_generation = EngineGeneration::NativeV2;
        task.status = status;
        task.proxy_binding = TaskProxyBinding::profile(None);
        upsert_download_task(pool, &task).await.unwrap();
        assert_eq!(
            clear_download_proxy(State(state.clone()))
                .await
                .unwrap_err()
                .status(),
            StatusCode::CONFLICT
        );
        task.proxy_binding = TaskProxyBinding::override_url("http://private.example:7890/".into());
        upsert_download_task(pool, &task).await.unwrap();
        assert_eq!(
            clear_download_proxy(State(state.clone())).await.unwrap(),
            StatusCode::NO_CONTENT
        );
    }
    let old_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_tasks WHERE engine_generation = 'legacy-v1' AND use_proxy = 1").fetch_one(pool).await.unwrap();
    assert_eq!(old_count, 1);
    cleanup_test_state(&state, runtime).await;
}
