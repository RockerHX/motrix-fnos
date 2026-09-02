use super::*;
use crate::app::{
    bootstrap_http_app_state, HttpAppState, ServerRuntimeConfig, DEFAULT_HTTP_ADDR,
    DEFAULT_JSONRPC_ADDR,
};
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::middleware;
use axum::response::Response;
use axum::routing::get;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tower::ServiceExt;

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

#[tokio::test]
async fn setup_and_login_return_bearer_tokens_without_cookies() {
    let state = test_state("login").await;
    let router = public_routes().with_state(state.clone());

    let setup = send(
        &router,
        "POST",
        "/auth/setup",
        Some(json!({"password": "correct horse battery"})),
        None,
    )
    .await;
    assert_eq!(setup.status(), StatusCode::OK);
    assert!(setup.headers().get("set-cookie").is_none());
    let setup_body = json_body(setup).await;
    let setup_token = setup_body["accessToken"].as_str().expect("setup token");
    assert!(!setup_token.is_empty());
    assert_eq!(setup_body["authenticated"], true);

    let status = send(&router, "GET", "/auth/status", None, Some(setup_token)).await;
    let status_body = json_body(status).await;
    assert_eq!(status_body["authenticated"], true);
    assert!(status_body.get("accessToken").is_none());

    let login = send(
        &router,
        "POST",
        "/auth/login",
        Some(json!({"password": "correct horse battery"})),
        None,
    )
    .await;
    assert_eq!(login.status(), StatusCode::OK);
    assert!(login.headers().get("set-cookie").is_none());
    assert!(json_body(login).await["accessToken"].is_string());
}

#[tokio::test]
async fn protected_management_requests_report_bearer_failure_codes() {
    let state = test_state("codes").await;
    let public = public_routes().with_state(state.clone());
    let setup = send(
        &public,
        "POST",
        "/auth/setup",
        Some(json!({"password": "correct horse battery"})),
        None,
    )
    .await;
    let token = json_body(setup).await["accessToken"]
        .as_str()
        .unwrap()
        .to_string();
    let protected = Router::new()
        .route("/protected", get(get_probe))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            management_auth,
        ))
        .with_state(state.clone());

    let missing = protected
        .clone()
        .oneshot(Request::get("/protected").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(json_body(missing).await["code"], "jwt_missing");

    let malformed = protected
        .clone()
        .oneshot(
            Request::get("/protected")
                .header("authorization", "Basic x")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(json_body(malformed).await["code"], "jwt_malformed");

    let valid = protected
        .clone()
        .oneshot(
            Request::get("/protected")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(valid.status(), StatusCode::OK);

    let invalid = protected
        .clone()
        .oneshot(
            Request::get("/protected")
                .header("authorization", format!("Bearer {}", tamper_token(&token)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(json_body(invalid).await["code"], "jwt_invalid");

    state
        .auth
        .service
        .change_password("correct horse battery", "replacement password")
        .await
        .expect("password should change");
    let version_mismatch = protected
        .oneshot(
            Request::get("/protected")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        json_body(version_mismatch).await["code"],
        "jwt_auth_version_mismatch"
    );
}

#[tokio::test]
async fn protection_disabled_allows_anonymous_management_and_sse_context() {
    let state = test_state("anonymous").await;
    let public = public_routes().with_state(state.clone());
    let setup = send(
        &public,
        "POST",
        "/auth/setup",
        Some(json!({"password": "correct horse battery"})),
        None,
    )
    .await;
    let token = json_body(setup).await["accessToken"]
        .as_str()
        .unwrap()
        .to_string();
    let changed = send(
        &public,
        "PUT",
        "/auth/protection",
        Some(json!({"enabled": false, "currentPassword": "correct horse battery"})),
        Some(&token),
    )
    .await;
    assert_eq!(changed.status(), StatusCode::OK);

    let protected = Router::new()
        .route("/protected", get(get_probe))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            management_auth,
        ))
        .with_state(state.clone());
    let anonymous = protected
        .oneshot(Request::get("/protected").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(anonymous.status(), StatusCode::OK);

    let events = Router::new()
        .route("/events", get(get_probe))
        .route_layer(middleware::from_fn_with_state(state, event_auth));
    let anonymous_event = events
        .oneshot(Request::get("/events").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(anonymous_event.status(), StatusCode::OK);
}

#[tokio::test]
async fn password_and_protection_changes_accept_current_password_without_a_bearer_token() {
    let state = test_state("invalidate").await;
    let public = public_routes().with_state(state.clone());
    let setup = send(
        &public,
        "POST",
        "/auth/setup",
        Some(json!({"password": "correct horse battery"})),
        None,
    )
    .await;
    let old_token = json_body(setup).await["accessToken"]
        .as_str()
        .unwrap()
        .to_string();
    let changed = send(&public, "PUT", "/auth/password", Some(json!({"currentPassword": "correct horse battery", "newPassword": "replacement password"})), None).await;
    assert_eq!(changed.status(), StatusCode::OK);

    let old_status = send(&public, "GET", "/auth/status", None, Some(&old_token)).await;
    assert_eq!(json_body(old_status).await["authenticated"], false);
    let new_token = json_body(changed).await["accessToken"]
        .as_str()
        .unwrap()
        .to_string();
    let new_status = send(&public, "GET", "/auth/status", None, Some(&new_token)).await;
    assert_eq!(json_body(new_status).await["authenticated"], true);

    let protection = send(
        &public,
        "PUT",
        "/auth/protection",
        Some(json!({"enabled": false, "currentPassword": "replacement password"})),
        None,
    )
    .await;
    assert_eq!(protection.status(), StatusCode::OK);
    assert!(json_body(protection).await["accessToken"].is_string());
}

#[tokio::test]
async fn auth_configuration_changes_reject_an_incorrect_current_password() {
    let state = test_state("configuration-password").await;
    let public = public_routes().with_state(state);
    let setup = send(
        &public,
        "POST",
        "/auth/setup",
        Some(json!({"password": "correct horse battery"})),
        None,
    )
    .await;
    assert_eq!(setup.status(), StatusCode::OK);

    for (uri, body) in [
        (
            "/auth/password",
            json!({"currentPassword": "wrong password", "newPassword": "replacement password"}),
        ),
        (
            "/auth/protection",
            json!({"enabled": false, "currentPassword": "wrong password"}),
        ),
    ] {
        let response = send(&public, "PUT", uri, Some(body), None).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(json_body(response).await["code"], "invalid_credentials");
    }
}

#[test]
fn jwt_failure_reasons_have_stable_error_codes() {
    for (failure, expected) in [
        (JwtValidationFailure::Malformed, "jwt_malformed"),
        (JwtValidationFailure::Invalid, "jwt_invalid"),
        (JwtValidationFailure::Expired, "jwt_expired"),
        (
            JwtValidationFailure::AuthVersionMismatch,
            "jwt_auth_version_mismatch",
        ),
        (
            JwtValidationFailure::InsufficientPrivileges,
            "jwt_insufficient_privileges",
        ),
    ] {
        assert_eq!(JwtFailureReason::from(failure).code(), expected);
    }
}

async fn get_probe() -> StatusCode {
    StatusCode::OK
}

fn tamper_token(token: &str) -> String {
    let (prefix, signature) = token
        .rsplit_once('.')
        .expect("JWT should contain a signature");
    let mut signature = signature.as_bytes().to_vec();
    signature[0] = if signature[0] == b'a' { b'b' } else { b'a' };
    format!(
        "{prefix}.{}",
        String::from_utf8(signature).expect("JWT should be UTF-8")
    )
}

async fn send(
    router: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    token: Option<&str>,
) -> Response {
    let mut builder = Request::builder().method(method).uri(uri);
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    router
        .clone()
        .oneshot(
            builder
                .body(Body::from(
                    body.map(|value| value.to_string()).unwrap_or_default(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn json_body(response: Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn test_state(label: &str) -> Arc<HttpAppState> {
    let app_data_dir = temp_dir(label);
    let runtime = ServerRuntimeConfig {
        database_path: app_data_dir.join("motrix-fnos.sqlite"),
        accessible_paths_path: app_data_dir.join("accessible-paths.json"),
        app_data_dir,
        http_addr: DEFAULT_HTTP_ADDR.parse().unwrap(),
        jsonrpc_addr: DEFAULT_JSONRPC_ADDR.parse().unwrap(),
        lan_jsonrpc_addr: "127.0.0.1:0".parse().unwrap(),
        aria2_path: None,
        trusted_proxy_ips: Vec::new(),
    };
    bootstrap_http_app_state(&runtime).await.unwrap()
}

fn temp_dir(label: &str) -> PathBuf {
    let index = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "motrix-fnos-auth-api-{label}-{}-{index}",
        std::process::id()
    ))
}
