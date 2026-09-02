use crate::api::error::ApiError;
use crate::api::extract::ApiJson;
use crate::api::RequestContext;
use crate::app::HttpAppState;
use crate::auth::{AuthError, AuthState, JwtValidationFailure, UNKNOWN_LOGIN_SOURCE};
use axum::body::Body;
use axum::extract::{ConnectInfo, State};
use axum::http::header::{AUTHORIZATION, CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, LazyLock};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

static LOGIN_DIAGNOSTIC_SLOTS: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(1)));

#[derive(Clone)]
pub(crate) struct EventAuthContext {
    pub(crate) token: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JwtFailureReason {
    Missing,
    Malformed,
    Invalid,
    Expired,
    AuthVersionMismatch,
    InsufficientPrivileges,
}

impl JwtFailureReason {
    fn code(self) -> &'static str {
        match self {
            Self::Missing => "jwt_missing",
            Self::Malformed => "jwt_malformed",
            Self::Invalid => "jwt_invalid",
            Self::Expired => "jwt_expired",
            Self::AuthVersionMismatch => "jwt_auth_version_mismatch",
            Self::InsufficientPrivileges => "jwt_insufficient_privileges",
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::Missing => "未收到管理访问令牌，请重新登录",
            Self::Malformed => "管理访问令牌格式无效，请重新登录",
            Self::Invalid => "管理访问令牌无效，请重新登录",
            Self::Expired => "管理访问令牌已过期，请重新登录",
            Self::AuthVersionMismatch => "管理访问令牌已失效，请重新登录",
            Self::InsufficientPrivileges => "管理访问令牌权限不足",
        }
    }
}

impl From<JwtValidationFailure> for JwtFailureReason {
    fn from(value: JwtValidationFailure) -> Self {
        match value {
            JwtValidationFailure::Malformed => Self::Malformed,
            JwtValidationFailure::Invalid => Self::Invalid,
            JwtValidationFailure::Expired => Self::Expired,
            JwtValidationFailure::AuthVersionMismatch => Self::AuthVersionMismatch,
            JwtValidationFailure::InsufficientPrivileges => Self::InsufficientPrivileges,
        }
    }
}

pub(crate) fn public_routes() -> Router<Arc<HttpAppState>> {
    Router::new()
        .route("/auth/status", get(status))
        .route("/auth/setup", post(setup))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/password", put(change_password))
        .route("/auth/protection", put(change_protection))
        .route("/auth/login-diagnostic", get(login_diagnostic))
}

pub(crate) async fn management_auth(
    State(state): State<Arc<HttpAppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let context = request.extensions().get::<RequestContext>().cloned();
    match authorize_management_request(&state, request.headers(), context.as_ref()).await {
        Ok(()) => next.run(request).await,
        Err(error) => error.into_response(),
    }
}

pub(crate) async fn event_auth(
    State(state): State<Arc<HttpAppState>>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let context = request.extensions().get::<RequestContext>().cloned();
    match authorize_event_request(&state, request.headers(), context.as_ref()).await {
        Ok(auth_context) => {
            request.extensions_mut().insert(auth_context);
            next.run(request).await
        }
        Err(error) => error.into_response(),
    }
}

pub(crate) async fn event_context_is_authorized(
    state: &HttpAppState,
    context: &EventAuthContext,
) -> bool {
    let Ok(auth_state) = load_auth_state(state).await else {
        return false;
    };
    if auth_state.setup_required {
        return false;
    }
    if !auth_state.enabled {
        return true;
    }
    let Some(token) = context.token.as_deref() else {
        return false;
    };
    state
        .auth
        .service
        .validate_admin_token(token, auth_state.auth_version)
        .await
        .is_ok()
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatusResponse {
    pub setup_required: bool,
    pub enabled: bool,
    pub authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
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
        return auth_status_response(&auth_state, false, None);
    }
    let authenticated = match bearer_token(&headers) {
        Ok(Some(token)) => state
            .auth
            .service
            .validate_admin_token(token, auth_state.auth_version)
            .await
            .is_ok(),
        Ok(None) | Err(_) => false,
    };
    auth_status_response(&auth_state, authenticated, None)
}

async fn setup(
    State(state): State<Arc<HttpAppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    ApiJson(payload): ApiJson<PasswordRequest>,
) -> Result<Response, ApiError> {
    let source = login_source(connect_info, &headers, &state.runtime.trusted_proxy_ips);
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
    let token = state
        .auth
        .service
        .issue_admin_token(&auth_state)
        .await
        .map_err(classify_auth_error)?;
    state
        .core
        .debug_logs
        .info("auth.setup", "Web 管理密码初始化成功");
    auth_status_response(&auth_state, true, Some(token))
}

async fn login(
    State(state): State<Arc<HttpAppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    ApiJson(payload): ApiJson<PasswordRequest>,
) -> Result<Response, ApiError> {
    let source = login_source(connect_info, &headers, &state.runtime.trusted_proxy_ips);
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
    let token = state
        .auth
        .service
        .issue_admin_token(&auth_state)
        .await
        .map_err(classify_auth_error)?;
    state.core.debug_logs.info("auth.login", "Web 管理登录成功");
    auth_status_response(&auth_state, true, Some(token))
}

async fn login_diagnostic(State(state): State<Arc<HttpAppState>>) -> Result<Response, ApiError> {
    let permit = Arc::clone(&*LOGIN_DIAGNOSTIC_SLOTS)
        .try_acquire_owned()
        .map_err(|_| {
            ApiError::too_many_requests(
                "login_diagnostic_busy",
                "已有登录诊断正在生成，请稍后重试",
                1,
            )
        })?;
    let bundle_state = Arc::clone(&state);
    let bundle = tokio::task::spawn_blocking(move || {
        let _permit: OwnedSemaphorePermit = permit;
        crate::diagnostics::build_login_diagnostic_bundle(&bundle_state)
    })
    .await
    .map_err(|error| {
        state.core.debug_logs.error(
            "diagnostics.login_bundle",
            format!("生成登录诊断包任务异常：{error}"),
        );
        ApiError::internal("login_diagnostic_failed", "生成登录诊断失败，请稍后重试")
    })?
    .map_err(|error| {
        state.core.debug_logs.error(
            "diagnostics.login_bundle",
            format!("生成登录诊断包失败：{error}"),
        );
        ApiError::internal("login_diagnostic_failed", "生成登录诊断失败，请稍后重试")
    })?;
    let mut response = Body::from(bundle).into_response();
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/zip"));
    response.headers_mut().insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"motrix-fnos-login-diagnostic.zip\""),
    );
    Ok(response)
}

async fn logout(State(state): State<Arc<HttpAppState>>) -> Result<Response, ApiError> {
    state
        .core
        .debug_logs
        .info("auth.logout", "Web 管理令牌已退出");
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn change_password(
    State(state): State<Arc<HttpAppState>>,
    ApiJson(payload): ApiJson<ChangePasswordRequest>,
) -> Result<Response, ApiError> {
    let auth_state = state
        .auth
        .service
        .change_password(&payload.current_password, &payload.new_password)
        .await
        .map_err(classify_auth_error)?;
    let token = state
        .auth
        .service
        .issue_admin_token(&auth_state)
        .await
        .map_err(classify_auth_error)?;
    state
        .core
        .debug_logs
        .info("auth.password", "Web 管理密码修改成功");
    auth_status_response(&auth_state, true, Some(token))
}

async fn change_protection(
    State(state): State<Arc<HttpAppState>>,
    ApiJson(payload): ApiJson<ChangeProtectionRequest>,
) -> Result<Response, ApiError> {
    let auth_state = state
        .auth
        .service
        .set_protection(payload.enabled, &payload.current_password)
        .await
        .map_err(classify_auth_error)?;
    let token = state
        .auth
        .service
        .issue_admin_token(&auth_state)
        .await
        .map_err(classify_auth_error)?;
    state.core.debug_logs.info(
        "auth.protection",
        if payload.enabled {
            "Web 管理访问保护已启用"
        } else {
            "Web 管理访问保护已关闭"
        },
    );
    auth_status_response(&auth_state, true, Some(token))
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
    context: Option<&RequestContext>,
) -> Result<(), ApiError> {
    let auth_state = load_auth_state(state).await?;
    if auth_state.setup_required {
        return Err(authentication_required());
    }
    if !auth_state.enabled {
        return Ok(());
    }
    let token = bearer_token(headers)
        .map_err(|reason| authentication_required_with_context(state, context, reason))?
        .ok_or_else(|| {
            authentication_required_with_context(state, context, JwtFailureReason::Missing)
        })?;
    state
        .auth
        .service
        .validate_admin_token(token, auth_state.auth_version)
        .await
        .map_err(|failure| authentication_required_with_context(state, context, failure.into()))?;
    Ok(())
}

async fn authorize_event_request(
    state: &HttpAppState,
    headers: &HeaderMap,
    context: Option<&RequestContext>,
) -> Result<EventAuthContext, ApiError> {
    let auth_state = load_auth_state(state).await?;
    if auth_state.setup_required {
        return Err(authentication_required());
    }
    if !auth_state.enabled {
        return Ok(EventAuthContext { token: None });
    }
    let token = bearer_token(headers)
        .map_err(|reason| authentication_required_with_context(state, context, reason))?
        .ok_or_else(|| {
            authentication_required_with_context(state, context, JwtFailureReason::Missing)
        })?;
    state
        .auth
        .service
        .validate_admin_token(token, auth_state.auth_version)
        .await
        .map_err(|failure| authentication_required_with_context(state, context, failure.into()))?;
    Ok(EventAuthContext {
        token: Some(token.to_string()),
    })
}

fn bearer_token(headers: &HeaderMap) -> Result<Option<&str>, JwtFailureReason> {
    let Some(value) = headers.get(AUTHORIZATION) else {
        return Ok(None);
    };
    let value = value.to_str().map_err(|_| JwtFailureReason::Malformed)?;
    let Some(token) = value.strip_prefix("Bearer ") else {
        return Err(JwtFailureReason::Malformed);
    };
    if token.trim().is_empty() || token.contains(char::is_whitespace) {
        return Err(JwtFailureReason::Malformed);
    }
    Ok(Some(token))
}

fn auth_status_response(
    auth_state: &AuthState,
    authenticated: bool,
    access_token: Option<String>,
) -> Result<Response, ApiError> {
    Ok(Json(AuthStatusResponse {
        setup_required: auth_state.setup_required,
        enabled: auth_state.enabled,
        authenticated,
        access_token,
    })
    .into_response())
}

fn login_source(
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: &HeaderMap,
    trusted_proxy_ips: &[IpAddr],
) -> String {
    let Some(ConnectInfo(address)) = connect_info else {
        return UNKNOWN_LOGIN_SOURCE.to_string();
    };

    if trusted_proxy_ips.contains(&address.ip()) {
        if let Some(forwarded_ip) = headers
            .get("x-forwarded-for")
            .and_then(|value| value.to_str().ok())
            .and_then(first_forwarded_ip)
        {
            return forwarded_ip.to_string();
        }
    }

    address.ip().to_string()
}

fn first_forwarded_ip(value: &str) -> Option<IpAddr> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .find_map(|item| item.parse::<IpAddr>().ok())
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
    ApiError::unauthorized("authentication_required", "需要有效的管理访问令牌")
}

fn authentication_required_with_context(
    state: &HttpAppState,
    context: Option<&RequestContext>,
    reason: JwtFailureReason,
) -> ApiError {
    let request_id = context
        .map(|value| value.request_id.as_str())
        .unwrap_or("unknown");
    let path = context
        .map(|value| value.path.as_str())
        .unwrap_or("unknown");
    state.core.debug_logs.warn(
        "auth.failure",
        format!(
            "request_id={request_id} path={path} auth_failure={}",
            reason.code()
        ),
    );
    ApiError::unauthorized_with_reason(reason.code(), reason.message(), reason.code())
}

fn rate_limited(seconds: u64) -> ApiError {
    ApiError::too_many_requests(
        "login_rate_limited",
        "登录失败次数过多，请稍后重试",
        seconds,
    )
}

fn auth_internal() -> ApiError {
    ApiError::internal("auth_unavailable", "Web 管理鉴权暂时不可用")
}

#[cfg(test)]
mod tests;
