use crate::api::error::ApiError;
use crate::api::extract::ApiJson;
use crate::app::HttpAppState;
use crate::auth::{
    clear_session_cookie, session_cookie, AuthError, AuthState, CreatedSession, SessionKind,
    ValidatedSession, SESSION_COOKIE_NAME, UNKNOWN_LOGIN_SOURCE,
};
use axum::body::Body;
use axum::extract::{ConnectInfo, State};
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, Method, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;

const CSRF_HEADER: &str = "x-csrf-token";

#[cfg(test)]
pub fn routes() -> Router<Arc<HttpAppState>> {
    Router::new()
        .merge(public_routes())
        .merge(session_routes())
        .merge(admin_routes())
}

pub(crate) fn public_routes() -> Router<Arc<HttpAppState>> {
    Router::new()
        .route("/auth/status", get(status))
        .route("/auth/setup", post(setup))
        .route("/auth/login", post(login))
}

pub(crate) fn session_routes() -> Router<Arc<HttpAppState>> {
    Router::new().route("/auth/logout", post(logout))
}

pub(crate) fn admin_routes() -> Router<Arc<HttpAppState>> {
    Router::new()
        .route("/auth/password", put(change_password))
        .route("/auth/protection", put(change_protection))
}

pub(crate) async fn management_auth(
    State(state): State<Arc<HttpAppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    match authorize_management_request(&state, request.headers(), request.method()).await {
        Ok(()) => next.run(request).await,
        Err(error) => error.into_response(),
    }
}

pub(crate) async fn event_auth(
    State(state): State<Arc<HttpAppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    match authorize_context_request(&state, request.headers(), false, false).await {
        Ok(()) => next.run(request).await,
        Err(error) => error.into_response(),
    }
}

pub(crate) async fn session_auth(
    State(state): State<Arc<HttpAppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    match authorize_context_request(&state, request.headers(), false, true).await {
        Ok(()) => next.run(request).await,
        Err(error) => error.into_response(),
    }
}

pub(crate) async fn admin_auth(
    State(state): State<Arc<HttpAppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    match authorize_context_request(&state, request.headers(), true, true).await {
        Ok(()) => next.run(request).await,
        Err(error) => error.into_response(),
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatusResponse {
    pub setup_required: bool,
    pub enabled: bool,
    pub authenticated: bool,
    pub csrf_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PasswordRequest {
    password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChangePasswordRequest {
    current_password: String,
    new_password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChangeProtectionRequest {
    enabled: bool,
    current_password: String,
}

async fn status(
    State(state): State<Arc<HttpAppState>>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let auth_state = load_auth_state(&state).await?;
    if auth_state.setup_required {
        return auth_status_response(&auth_state, None, None);
    }

    let existing = validated_session(&state, &headers, &auth_state)?;
    if auth_state.enabled {
        return auth_status_response(&auth_state, existing, None);
    }
    if existing.is_some() {
        return auth_status_response(&auth_state, existing, None);
    }

    let session = state
        .auth
        .sessions
        .create(SessionKind::AnonymousManagement, auth_state.auth_version)
        .map_err(session_error)?;
    auth_status_response(&auth_state, None, Some(session))
}

async fn setup(
    State(state): State<Arc<HttpAppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    ApiJson(payload): ApiJson<PasswordRequest>,
) -> Result<Response, ApiError> {
    let source = login_source(connect_info);
    let auth_state = state
        .auth
        .service
        .setup(&payload.password)
        .await
        .map_err(classify_auth_error)?;
    state
        .auth
        .login_limiter
        .record_success(&source)
        .map_err(|_| auth_internal())?;
    state.auth.sessions.revoke_all().map_err(session_error)?;
    let session = state
        .auth
        .sessions
        .create(SessionKind::Admin, auth_state.auth_version)
        .map_err(session_error)?;
    state
        .core
        .debug_logs
        .info("auth.setup", "Web 管理密码初始化成功");
    auth_status_response(&auth_state, None, Some(session))
}

async fn login(
    State(state): State<Arc<HttpAppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    ApiJson(payload): ApiJson<PasswordRequest>,
) -> Result<Response, ApiError> {
    let source = login_source(connect_info);
    if let Some(seconds) = state
        .auth
        .login_limiter
        .retry_after_seconds(&source)
        .map_err(|_| auth_internal())?
    {
        return Err(rate_limited(seconds));
    }

    let auth_state = match state.auth.service.verify_password(&payload.password).await {
        Ok(auth_state) => auth_state,
        Err(AuthError::InvalidCredentials) => {
            state.core.debug_logs.warn("auth.login", "Web 管理登录失败");
            if let Some(seconds) = state
                .auth
                .login_limiter
                .record_failure(&source)
                .map_err(|_| auth_internal())?
            {
                return Err(rate_limited(seconds));
            }
            return Err(invalid_credentials());
        }
        Err(error) => return Err(classify_auth_error(error)),
    };
    state
        .auth
        .login_limiter
        .record_success(&source)
        .map_err(|_| auth_internal())?;
    let session = state
        .auth
        .sessions
        .create(SessionKind::Admin, auth_state.auth_version)
        .map_err(session_error)?;
    state.core.debug_logs.info("auth.login", "Web 管理登录成功");
    auth_status_response(&auth_state, None, Some(session))
}

async fn logout(
    State(state): State<Arc<HttpAppState>>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let auth_state = load_auth_state(&state).await?;
    let session = require_session_with_csrf(&state, &headers, &auth_state, false)?;
    state
        .auth
        .sessions
        .revoke(&session.id)
        .map_err(session_error)?;
    state
        .core
        .debug_logs
        .info("auth.logout", "Web 管理会话已退出");
    response_with_cookie(
        StatusCode::NO_CONTENT.into_response(),
        clear_session_cookie(),
    )
}

async fn change_password(
    State(state): State<Arc<HttpAppState>>,
    headers: HeaderMap,
    ApiJson(payload): ApiJson<ChangePasswordRequest>,
) -> Result<Response, ApiError> {
    let current = load_auth_state(&state).await?;
    require_session_with_csrf(&state, &headers, &current, true)?;
    let auth_state = state
        .auth
        .service
        .change_password(&payload.current_password, &payload.new_password)
        .await
        .map_err(classify_auth_error)?;
    replace_admin_session(&state, &auth_state, "auth.password", "Web 管理密码修改成功")
}

async fn change_protection(
    State(state): State<Arc<HttpAppState>>,
    headers: HeaderMap,
    ApiJson(payload): ApiJson<ChangeProtectionRequest>,
) -> Result<Response, ApiError> {
    let current = load_auth_state(&state).await?;
    require_session_with_csrf(&state, &headers, &current, true)?;
    let auth_state = state
        .auth
        .service
        .set_protection(payload.enabled, &payload.current_password)
        .await
        .map_err(classify_auth_error)?;
    replace_admin_session(
        &state,
        &auth_state,
        "auth.protection",
        if payload.enabled {
            "Web 管理访问保护已启用"
        } else {
            "Web 管理访问保护已关闭"
        },
    )
}

fn replace_admin_session(
    state: &HttpAppState,
    auth_state: &AuthState,
    log_module: &str,
    log_message: &str,
) -> Result<Response, ApiError> {
    state.auth.sessions.revoke_all().map_err(session_error)?;
    let session = state
        .auth
        .sessions
        .create(SessionKind::Admin, auth_state.auth_version)
        .map_err(session_error)?;
    state.core.debug_logs.info(log_module, log_message);
    auth_status_response(auth_state, None, Some(session))
}

async fn load_auth_state(state: &HttpAppState) -> Result<AuthState, ApiError> {
    state
        .auth
        .service
        .state()
        .await
        .map_err(classify_auth_error)
}

async fn authorize_management_request(
    state: &HttpAppState,
    headers: &HeaderMap,
    method: &Method,
) -> Result<(), ApiError> {
    let auth_state = load_auth_state(state).await?;
    if auth_state.setup_required {
        return Err(authentication_required());
    }
    if auth_state.enabled {
        let session = validated_session(state, headers, &auth_state)?
            .filter(|session| session.kind == SessionKind::Admin)
            .ok_or_else(authentication_required)?;
        if is_write_method(method) {
            validate_csrf(state, headers, &auth_state, &session)?;
        }
        return Ok(());
    }
    if !is_write_method(method) {
        return Ok(());
    }
    let session =
        validated_session(state, headers, &auth_state)?.ok_or_else(authentication_required)?;
    validate_csrf(state, headers, &auth_state, &session)
}

async fn authorize_context_request(
    state: &HttpAppState,
    headers: &HeaderMap,
    admin_only: bool,
    require_csrf: bool,
) -> Result<(), ApiError> {
    let auth_state = load_auth_state(state).await?;
    if auth_state.setup_required {
        return Err(authentication_required());
    }
    let session =
        validated_session(state, headers, &auth_state)?.ok_or_else(authentication_required)?;
    if admin_only && session.kind != SessionKind::Admin {
        return Err(ApiError::unauthorized(
            "admin_authentication_required",
            "需要管理员登录会话",
        ));
    }
    if auth_state.enabled && session.kind != SessionKind::Admin {
        return Err(authentication_required());
    }
    if require_csrf {
        validate_csrf(state, headers, &auth_state, &session)?;
    }
    Ok(())
}

fn validate_csrf(
    state: &HttpAppState,
    headers: &HeaderMap,
    auth_state: &AuthState,
    session: &ValidatedSession,
) -> Result<(), ApiError> {
    let csrf = headers
        .get(CSRF_HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if !state
        .auth
        .sessions
        .validate_csrf(&session.id, auth_state.auth_version, csrf)
        .map_err(session_error)?
    {
        return Err(ApiError::forbidden("csrf_invalid", "CSRF Token 缺失或无效"));
    }
    Ok(())
}

fn is_write_method(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

fn require_session_with_csrf(
    state: &HttpAppState,
    headers: &HeaderMap,
    auth_state: &AuthState,
    admin_only: bool,
) -> Result<ValidatedSession, ApiError> {
    let session = validated_session(state, headers, auth_state)?
        .ok_or_else(|| ApiError::unauthorized("authentication_required", "需要有效的管理会话"))?;
    if admin_only && session.kind != SessionKind::Admin {
        return Err(ApiError::unauthorized(
            "admin_authentication_required",
            "需要管理员登录会话",
        ));
    }
    let csrf = headers
        .get(CSRF_HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if !state
        .auth
        .sessions
        .validate_csrf(&session.id, auth_state.auth_version, csrf)
        .map_err(session_error)?
    {
        return Err(ApiError::forbidden("csrf_invalid", "CSRF Token 缺失或无效"));
    }
    Ok(session)
}

fn validated_session(
    state: &HttpAppState,
    headers: &HeaderMap,
    auth_state: &AuthState,
) -> Result<Option<ValidatedSession>, ApiError> {
    let Some(session_id) = session_id(headers) else {
        return Ok(None);
    };
    state
        .auth
        .sessions
        .validate(session_id, auth_state.auth_version)
        .map_err(session_error)
}

fn session_id(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|cookie| cookie.trim().split_once('='))
        .find_map(|(name, value)| (name == SESSION_COOKIE_NAME).then_some(value))
}

fn login_source(connect_info: Option<ConnectInfo<SocketAddr>>) -> String {
    connect_info
        .map(|ConnectInfo(address)| address.ip().to_string())
        .unwrap_or_else(|| UNKNOWN_LOGIN_SOURCE.to_string())
}

fn auth_status_response(
    auth_state: &AuthState,
    existing: Option<ValidatedSession>,
    created: Option<CreatedSession>,
) -> Result<Response, ApiError> {
    let authenticated = created
        .as_ref()
        .map(|session| session.kind == SessionKind::Admin)
        .or_else(|| {
            existing
                .as_ref()
                .map(|session| session.kind == SessionKind::Admin)
        })
        .unwrap_or(false);
    let csrf_token = created
        .as_ref()
        .map(|session| session.csrf_token.clone())
        .or_else(|| existing.as_ref().map(|session| session.csrf_token.clone()));
    let response = Json(AuthStatusResponse {
        setup_required: auth_state.setup_required,
        enabled: auth_state.enabled,
        authenticated,
        csrf_token,
    })
    .into_response();
    match created {
        Some(session) => response_with_cookie(response, session_cookie(&session.id)),
        None => Ok(response),
    }
}

fn response_with_cookie(mut response: Response, cookie: String) -> Result<Response, ApiError> {
    let value = HeaderValue::from_str(&cookie).map_err(|_| auth_internal())?;
    response.headers_mut().insert(SET_COOKIE, value);
    Ok(response)
}

fn classify_auth_error(error: AuthError) -> ApiError {
    match error {
        AuthError::AlreadyInitialized => {
            ApiError::conflict("auth_already_initialized", "Web 管理密码已经初始化")
        }
        AuthError::InvalidCredentials => invalid_credentials(),
        AuthError::InvalidPassword(message) => ApiError::bad_request("invalid_password", message),
        AuthError::InvalidState(_) | AuthError::Storage(_) => auth_internal(),
    }
}

fn invalid_credentials() -> ApiError {
    ApiError::unauthorized("invalid_credentials", "管理密码无效")
}

fn authentication_required() -> ApiError {
    ApiError::unauthorized("authentication_required", "需要有效的管理会话")
}

fn rate_limited(seconds: u64) -> ApiError {
    ApiError::too_many_requests(
        "login_rate_limited",
        "登录失败次数过多，请稍后重试",
        seconds,
    )
}

fn session_error(_: crate::auth::SessionError) -> ApiError {
    auth_internal()
}

fn auth_internal() -> ApiError {
    ApiError::internal("auth_unavailable", "Web 管理鉴权暂时不可用")
}

#[cfg(test)]
mod tests;
