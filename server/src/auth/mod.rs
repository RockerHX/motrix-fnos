mod jwt;
mod password;
mod process_lock;
mod rate_limit;

use crate::database::web_auth::{self, WebAuthRow};
use password::{hash_password, validate_password, verify_password_hash, PasswordHashSlots};
use sqlx::SqlitePool;
use std::time::{SystemTime, UNIX_EPOCH};

const PASSWORD_HASH_CONCURRENCY: usize = 2;

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
        let password_hash_slots = PasswordHashSlots::new(PASSWORD_HASH_CONCURRENCY);
        Self {
            service: AuthService::with_password_hash_slots(pool, password_hash_slots.clone()),
            login_limiter: LoginRateLimiter::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthState {
    pub setup_required: bool,
    pub auth_version: u64,
    pub password_updated_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    AlreadyInitialized,
    InvalidCredentials,
    InvalidPassword(String),
    PasswordHashBusy,
    PasswordChanged,
    InvalidState(String),
    Storage(String),
}

#[derive(Clone)]
pub struct AuthService {
    pool: SqlitePool,
    password_hash_slots: PasswordHashSlots,
}

impl AuthService {
    pub fn new(pool: SqlitePool) -> Self {
        Self::with_password_hash_slots(pool, PasswordHashSlots::new(PASSWORD_HASH_CONCURRENCY))
    }

    fn with_password_hash_slots(pool: SqlitePool, password_hash_slots: PasswordHashSlots) -> Self {
        Self {
            pool,
            password_hash_slots,
        }
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
        let existing = validated_record(
            web_auth::load(&self.pool)
                .await
                .map_err(AuthError::Storage)?,
        )?;
        if existing.is_configured() {
            return Err(AuthError::AlreadyInitialized);
        }
        let auth_version = if existing.exists {
            next_auth_version(existing.auth_version)?
        } else {
            1
        };
        let password_hash = self
            .password_hash_slots
            .run({
                let password = password.to_string();
                move || hash_password(&password)
            })
            .await?;
        let jwt_secret = existing.jwt_secret.unwrap_or_else(jwt::generate_secret);
        let password_updated_at = current_timestamp_ms()?;
        let initialized = web_auth::initialize_password(
            &self.pool,
            &password_hash,
            password_updated_at,
            existing.exists,
            &jwt_secret,
        )
        .await
        .map_err(AuthError::Storage)?;
        if !initialized {
            return Err(AuthError::AlreadyInitialized);
        }
        Ok(AuthState {
            setup_required: false,
            auth_version,
            password_updated_at: Some(password_updated_at),
        })
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
        let password_hash = password_hash.to_string();
        let password = password.to_string();
        if !self
            .password_hash_slots
            .run(move || Ok(verify_password_hash(&password, &password_hash)))
            .await?
        {
            return Err(AuthError::InvalidCredentials);
        }
        Ok(record.state())
    }

    pub async fn change_password(
        &self,
        current_password: &str,
        new_password: &str,
    ) -> Result<AuthState, AuthError> {
        let record = validated_record(
            web_auth::load(&self.pool)
                .await
                .map_err(AuthError::Storage)?,
        )?;
        let Some(password_hash) = record.password_hash else {
            return Err(AuthError::InvalidCredentials);
        };
        let auth_version = next_auth_version(record.auth_version)?;
        let current_password = current_password.to_string();
        let new_password = new_password.to_string();
        let password_hash_for_verify = password_hash.clone();
        let new_hash = self
            .password_hash_slots
            .run(move || {
                if !verify_password_hash(&current_password, &password_hash_for_verify) {
                    return Ok(None);
                }
                validate_password(&new_password)?;
                hash_password(&new_password).map(Some)
            })
            .await?
            .ok_or(AuthError::InvalidCredentials)?;
        let password_updated_at = current_timestamp_ms()?;
        let updated = web_auth::update_password(
            &self.pool,
            &new_hash,
            password_updated_at,
            record.auth_version as i64,
            &password_hash,
        )
        .await
        .map_err(AuthError::Storage)?;
        if !updated {
            return Err(AuthError::PasswordChanged);
        }
        Ok(AuthState {
            setup_required: false,
            auth_version,
            password_updated_at: Some(password_updated_at),
        })
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
            auth_version: self.auth_version,
            password_updated_at: self.password_updated_at,
        }
    }
}

fn validated_record(row: Option<WebAuthRow>) -> Result<ValidatedAuthRecord, AuthError> {
    let Some(row) = row else {
        return Ok(ValidatedAuthRecord {
            exists: false,
            password_hash: None,
            password_updated_at: None,
            auth_version: 0,
            jwt_secret: None,
        });
    };
    match row.enabled {
        0 | 1 => {}
        _ => return Err(invalid_state("enabled 字段非法")),
    }
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

fn next_auth_version(current: u64) -> Result<u64, AuthError> {
    current
        .checked_add(1)
        .filter(|version| *version <= i64::MAX as u64)
        .ok_or_else(|| invalid_state("auth_version 超出范围"))
}

fn invalid_state(message: &str) -> AuthError {
    AuthError::InvalidState(message.to_string())
}

#[cfg(test)]
mod tests;
