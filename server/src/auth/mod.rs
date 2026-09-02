mod jwt;
mod password;
mod process_lock;
mod rate_limit;

use crate::database::web_auth::{self, WebAuthRow};
use password::{hash_password, validate_password, verify_password_hash};
use sqlx::SqlitePool;
use std::time::{SystemTime, UNIX_EPOCH};

pub use jwt::{
    generate_secret as generate_jwt_secret, Claims, JwtValidationFailure, JWT_LIFETIME_SECONDS,
};
pub use process_lock::ServerProcessLock;
pub use rate_limit::{LoginRateLimitError, LoginRateLimiter, UNKNOWN_LOGIN_SOURCE};

#[derive(Clone)]
pub struct AuthRuntime {
    pub service: AuthService,
    pub login_limiter: LoginRateLimiter,
}

impl AuthRuntime {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            service: AuthService::new(pool),
            login_limiter: LoginRateLimiter::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthState {
    pub setup_required: bool,
    pub enabled: bool,
    pub auth_version: u64,
    pub password_updated_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    AlreadyInitialized,
    InvalidCredentials,
    InvalidPassword(String),
    InvalidState(String),
    Storage(String),
}

#[derive(Clone)]
pub struct AuthService {
    pool: SqlitePool,
}

impl AuthService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn state(&self) -> Result<AuthState, AuthError> {
        validated_record(
            web_auth::load(&self.pool)
                .await
                .map_err(AuthError::Storage)?,
        )
        .map(|record| record.state())
    }

    pub async fn setup(&self, password: &str) -> Result<AuthState, AuthError> {
        validate_password(password)?;
        let password_hash = hash_password(password)?;
        let existing = validated_record(
            web_auth::load(&self.pool)
                .await
                .map_err(AuthError::Storage)?,
        )?;
        if existing.is_configured() {
            return Err(AuthError::AlreadyInitialized);
        }
        let jwt_secret = existing.jwt_secret.unwrap_or_else(jwt::generate_secret);
        let initialized = web_auth::initialize_password(
            &self.pool,
            &password_hash,
            current_timestamp_ms()?,
            existing.exists,
            &jwt_secret,
        )
        .await
        .map_err(AuthError::Storage)?;
        if !initialized {
            return Err(AuthError::AlreadyInitialized);
        }
        self.state().await
    }

    pub async fn verify_password(&self, password: &str) -> Result<AuthState, AuthError> {
        let record = validated_record(
            web_auth::load(&self.pool)
                .await
                .map_err(AuthError::Storage)?,
        )?;
        let Some(password_hash) = record.password_hash.as_deref() else {
            return Err(AuthError::InvalidCredentials);
        };
        if !verify_password_hash(password, password_hash) {
            return Err(AuthError::InvalidCredentials);
        }
        Ok(record.state())
    }

    pub async fn change_password(
        &self,
        current_password: &str,
        new_password: &str,
    ) -> Result<AuthState, AuthError> {
        validate_password(new_password)?;
        let new_hash = hash_password(new_password)?;
        let mut transaction = self.pool.begin().await.map_err(storage_error)?;
        let record = validated_record(
            web_auth::load_in_transaction(&mut transaction)
                .await
                .map_err(AuthError::Storage)?,
        )?;
        let Some(password_hash) = record.password_hash.as_deref() else {
            return Err(AuthError::InvalidCredentials);
        };
        if !verify_password_hash(current_password, password_hash) {
            return Err(AuthError::InvalidCredentials);
        }
        web_auth::update_password(&mut transaction, &new_hash, current_timestamp_ms()?)
            .await
            .map_err(AuthError::Storage)?;
        transaction.commit().await.map_err(storage_error)?;
        self.state().await
    }

    pub async fn set_protection(
        &self,
        enabled: bool,
        current_password: &str,
    ) -> Result<AuthState, AuthError> {
        let mut transaction = self.pool.begin().await.map_err(storage_error)?;
        let record = validated_record(
            web_auth::load_in_transaction(&mut transaction)
                .await
                .map_err(AuthError::Storage)?,
        )?;
        let Some(password_hash) = record.password_hash.as_deref() else {
            return Err(AuthError::InvalidCredentials);
        };
        if !verify_password_hash(current_password, password_hash) {
            return Err(AuthError::InvalidCredentials);
        }
        web_auth::update_protection(&mut transaction, enabled)
            .await
            .map_err(AuthError::Storage)?;
        transaction.commit().await.map_err(storage_error)?;
        self.state().await
    }

    pub async fn reset(&self) -> Result<(), AuthError> {
        let jwt_secret = web_auth::load(&self.pool)
            .await
            .map_err(AuthError::Storage)?
            .and_then(|row| row.jwt_secret)
            .unwrap_or_else(jwt::generate_secret);
        web_auth::reset(&self.pool, &jwt_secret)
            .await
            .map_err(AuthError::Storage)
    }

    pub async fn issue_admin_token(&self, auth_state: &AuthState) -> Result<String, AuthError> {
        let record = validated_record(
            web_auth::load(&self.pool)
                .await
                .map_err(AuthError::Storage)?,
        )?;
        let secret = record
            .jwt_secret
            .ok_or_else(|| invalid_state("JWT 密钥缺失"))?;
        jwt::issue_now(&secret, auth_state.auth_version).map_err(AuthError::Storage)
    }

    pub async fn validate_admin_token(
        &self,
        token: &str,
        auth_version: u64,
    ) -> Result<Claims, JwtValidationFailure> {
        let row = web_auth::load(&self.pool)
            .await
            .map_err(|_| JwtValidationFailure::Invalid)?;
        let record = validated_record(row).map_err(|_| JwtValidationFailure::Invalid)?;
        let secret = record.jwt_secret.ok_or(JwtValidationFailure::Invalid)?;
        jwt::validate(&secret, token, auth_version)
    }
}

struct ValidatedAuthRecord {
    exists: bool,
    enabled: bool,
    password_hash: Option<String>,
    password_updated_at: Option<i64>,
    auth_version: u64,
    jwt_secret: Option<String>,
}

impl ValidatedAuthRecord {
    fn is_configured(&self) -> bool {
        self.password_hash.is_some()
    }

    fn state(&self) -> AuthState {
        AuthState {
            setup_required: !self.is_configured(),
            enabled: self.enabled,
            auth_version: self.auth_version,
            password_updated_at: self.password_updated_at,
        }
    }
}

fn validated_record(row: Option<WebAuthRow>) -> Result<ValidatedAuthRecord, AuthError> {
    let Some(row) = row else {
        return Ok(ValidatedAuthRecord {
            exists: false,
            enabled: true,
            password_hash: None,
            password_updated_at: None,
            auth_version: 0,
            jwt_secret: None,
        });
    };
    let enabled = match row.enabled {
        0 => false,
        1 => true,
        _ => return Err(invalid_state("enabled 字段非法")),
    };
    let auth_version = u64::try_from(row.auth_version)
        .ok()
        .filter(|version| *version > 0)
        .ok_or_else(|| invalid_state("auth_version 字段非法"))?;
    match (&row.password_hash, row.password_updated_at) {
        (None, None) => {}
        (Some(hash), Some(updated_at)) if !hash.is_empty() && updated_at > 0 => {}
        _ => return Err(invalid_state("密码字段组合不完整")),
    }
    if row.jwt_secret.as_deref().is_none_or(str::is_empty) {
        return Err(invalid_state("JWT 密钥缺失"));
    }
    Ok(ValidatedAuthRecord {
        exists: true,
        enabled,
        password_hash: row.password_hash,
        password_updated_at: row.password_updated_at,
        auth_version,
        jwt_secret: row.jwt_secret,
    })
}

fn current_timestamp_ms() -> Result<i64, AuthError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| invalid_state(&format!("系统时间非法：{error}")))?
        .as_millis();
    i64::try_from(millis).map_err(|_| invalid_state("系统时间超出范围"))
}

fn invalid_state(message: &str) -> AuthError {
    AuthError::InvalidState(message.to_string())
}

fn storage_error(error: sqlx::Error) -> AuthError {
    AuthError::Storage(format!("Web 鉴权数据库事务失败：{error}"))
}

#[cfg(test)]
mod tests;
