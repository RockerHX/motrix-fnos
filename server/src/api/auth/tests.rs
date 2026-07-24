use super::*;
use crate::app::{
    bootstrap_http_app_state, ServerRuntimeConfig, DEFAULT_HTTP_ADDR, DEFAULT_JSONRPC_ADDR,
};
use axum::body::{to_bytes, Body};
use axum::extract::ConnectInfo;
use axum::http::Request;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use tower::ServiceExt;

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

#[tokio::test]
async fn auth_api_supports_setup_logout_password_and_protection_lifecycle() {
    let state = test_state("lifecycle").await;
    let router = routes().with_state(state.clone());

    let initial = send(&router, "GET", "/auth/status", None, None, None).await;
    assert_eq!(initial.status(), StatusCode::OK);
    assert_eq!(json_body(initial).await["setupRequired"], true);

    let setup = send(
        &router,
        "POST",
        "/auth/setup",
        Some(json!({ "password": "correct horse battery" })),
        None,
        None,
    )
    .await;
    assert_eq!(setup.status(), StatusCode::OK);
    let first_cookie = response_cookie(&setup);
    let setup_body = json_body(setup).await;
    let first_csrf = setup_body["csrfToken"].as_str().expect("csrf should exist");
    assert_eq!(setup_body["authenticated"], true);

    let duplicate = send(
        &router,
        "POST",
        "/auth/setup",
        Some(json!({ "password": "another secure password" })),
        None,
        None,
    )
    .await;
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);

    let missing_csrf = send(
        &router,
        "POST",
        "/auth/logout",
        None,
        Some(&first_cookie),
        None,
    )
    .await;
    assert_eq!(missing_csrf.status(), StatusCode::FORBIDDEN);

    let changed = send(
        &router,
        "PUT",
        "/auth/password",
        Some(json!({
            "currentPassword": "correct horse battery",
            "newPassword": "replacement password"
        })),
        Some(&first_cookie),
        Some(first_csrf),
    )
    .await;
    assert_eq!(changed.status(), StatusCode::OK);
    let second_cookie = response_cookie(&changed);
    let changed_body = json_body(changed).await;
    let second_csrf = changed_body["csrfToken"]
        .as_str()
        .expect("csrf should exist");
    assert_ne!(first_cookie, second_cookie);

    let old_status = send(
        &router,
        "GET",
        "/auth/status",
        None,
        Some(&first_cookie),
        None,
    )
    .await;
    assert_eq!(json_body(old_status).await["authenticated"], false);

    let disabled = send(
        &router,
        "PUT",
        "/auth/protection",
        Some(json!({
            "enabled": false,
            "currentPassword": "replacement password"
        })),
        Some(&second_cookie),
        Some(second_csrf),
    )
    .await;
    assert_eq!(disabled.status(), StatusCode::OK);
    let disabled_body = json_body(disabled).await;
    assert_eq!(disabled_body["enabled"], false);
    assert_eq!(disabled_body["authenticated"], true);

    let anonymous = send(&router, "GET", "/auth/status", None, None, None).await;
    assert!(anonymous.headers().contains_key(SET_COOKIE));
    let anonymous_body = json_body(anonymous).await;
    assert_eq!(anonymous_body["authenticated"], false);
    assert!(anonymous_body["csrfToken"].is_string());
}

#[tokio::test]
async fn login_uses_generic_errors_and_rate_limit() {
    let state = test_state("rate-limit").await;
    let router = routes().with_state(state);
    let setup = send(
        &router,
        "POST",
        "/auth/setup",
        Some(json!({ "password": "correct horse battery" })),
        None,
        None,
    )
    .await;
    assert_eq!(setup.status(), StatusCode::OK);

    for _ in 0..4 {
        let failed = send(
            &router,
            "POST",
            "/auth/login",
            Some(json!({ "password": "incorrect password" })),
            None,
            None,
        )
        .await;
        assert_eq!(failed.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(json_body(failed).await["code"], "invalid_credentials");
    }
    let limited = send(
        &router,
        "POST",
        "/auth/login",
        Some(json!({ "password": "incorrect password" })),
        None,
        None,
    )
    .await;
    assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(limited.headers()["retry-after"], "30");
    assert_eq!(json_body(limited).await["code"], "login_rate_limited");
}

#[tokio::test]
async fn login_rate_limit_isolated_by_connect_source() {
    let state = test_state("rate-limit-source").await;
    let router = routes().with_state(state);
    let setup = send_from(
        &router,
        "POST",
        "/auth/setup",
        Some(json!({ "password": "correct horse battery" })),
        None,
        None,
        Some("192.0.2.10:1000"),
    )
    .await;
    assert_eq!(setup.status(), StatusCode::OK);

    for _ in 0..4 {
        let failed = send_from(
            &router,
            "POST",
            "/auth/login",
            Some(json!({ "password": "incorrect password" })),
            None,
            None,
            Some("192.0.2.10:1000"),
        )
        .await;
        assert_eq!(failed.status(), StatusCode::UNAUTHORIZED);
    }
    let other_source = send_from(
        &router,
        "POST",
        "/auth/login",
        Some(json!({ "password": "incorrect password" })),
        None,
        None,
        Some("192.0.2.11:1000"),
    )
    .await;
    assert_eq!(other_source.status(), StatusCode::UNAUTHORIZED);

    let locked = send_from(
        &router,
        "POST",
        "/auth/login",
        Some(json!({ "password": "incorrect password" })),
        None,
        None,
        Some("192.0.2.10:2000"),
    )
    .await;
    assert_eq!(locked.status(), StatusCode::TOO_MANY_REQUESTS);

    let other_source_again = send_from(
        &router,
        "POST",
        "/auth/login",
        Some(json!({ "password": "incorrect password" })),
        None,
        None,
        Some("192.0.2.11:2000"),
    )
    .await;
    assert_eq!(other_source_again.status(), StatusCode::UNAUTHORIZED);
}

async fn send(
    router: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    cookie: Option<&str>,
    csrf: Option<&str>,
) -> Response {
    send_from(router, method, uri, body, cookie, csrf, None).await
}

async fn send_from(
    router: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    cookie: Option<&str>,
    csrf: Option<&str>,
    source: Option<&str>,
) -> Response {
    let mut builder = Request::builder().method(method).uri(uri);
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    if let Some(cookie) = cookie {
        builder = builder.header(COOKIE, cookie);
    }
    if let Some(csrf) = csrf {
        builder = builder.header(CSRF_HEADER, csrf);
    }
    let mut request = builder
        .body(Body::from(
            body.map(|value| value.to_string()).unwrap_or_default(),
        ))
        .expect("request should build");
    if let Some(source) = source {
        request.extensions_mut().insert(ConnectInfo(
            source
                .parse::<std::net::SocketAddr>()
                .expect("source should parse"),
        ));
    }
    router
        .clone()
        .oneshot(request)
        .await
        .expect("request should complete")
}

fn response_cookie(response: &Response) -> String {
    response
        .headers()
        .get(SET_COOKIE)
        .expect("set-cookie should exist")
        .to_str()
        .expect("cookie should be text")
        .split(';')
        .next()
        .expect("cookie pair should exist")
        .to_string()
}

async fn json_body(response: Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should read");
    serde_json::from_slice(&bytes).expect("body should be json")
}

async fn test_state(label: &str) -> Arc<HttpAppState> {
    let app_data_dir = temp_dir(label);
    let runtime = ServerRuntimeConfig {
        database_path: app_data_dir.join("motrix-fnos.sqlite"),
        accessible_paths_path: app_data_dir.join("accessible-paths.json"),
        app_data_dir,
        http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
        jsonrpc_addr: DEFAULT_JSONRPC_ADDR.parse().expect("addr should parse"),
        aria2_path: None,
    };
    bootstrap_http_app_state(&runtime)
        .await
        .expect("state should bootstrap")
}

fn temp_dir(label: &str) -> PathBuf {
    let index = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "motrix-fnos-auth-api-{label}-{}-{index}",
        std::process::id()
    ))
}
