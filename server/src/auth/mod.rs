mod jwt;
mod password;
mod process_lock;
mod rate_limit;

use crate::database::web_auth::{self, WebAuthRow};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use password::{hash_password, validate_password, verify_password_hash, PasswordHashSlots};
use rand_core::{OsRng, RngCore};
use sqlx::SqlitePool;
use std::time::{SystemTime, UNIX_EPOCH};

const PASSWORD_HASH_CONCURRENCY: usize = 2;
pub const BOOTSTRAP_TOKEN_TTL_SECONDS: i64 = 15 * 60;

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
    BootstrapTokenInvalid,
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

    pub async fn initialize_password(&self, password: &str) -> Result<AuthState, AuthError> {
        let record = validated_record(
            web_auth::load(&self.pool)
                .await
                .map_err(AuthError::Storage)?,
        )?;
        if record.is_configured() {
            return Err(AuthError::AlreadyInitialized);
        }
        next_auth_version(record.auth_version)?;
        validate_password(password)?;
        let password_hash = self
            .password_hash_slots
            .run({
                let password = password.to_string();
                move || hash_password(&password)
            })
            .await?;
        let password_updated_at = current_timestamp_ms()?;
        let auth_version = web_auth::initialize_password(
            &self.pool,
            &password_hash,
            password_updated_at,
            &record.jwt_secret.unwrap_or_else(jwt::generate_secret),
        )
        .await
        .map_err(AuthError::Storage)?
        .ok_or(AuthError::AlreadyInitialized)?;
        Ok(AuthState {
            setup_required: false,
            auth_version: auth_version as u64,
            password_updated_at: Some(password_updated_at),
        })
    }

    pub async fn setup_with_bootstrap_token(
        &self,
        bootstrap_token: &str,
        password: &str,
    ) -> Result<AuthState, AuthError> {
        let bootstrap_token_hash = self.verify_bootstrap_token(bootstrap_token).await?;
        validate_password(password)?;
        let password_hash = self
            .password_hash_slots
            .run({
                let password = password.to_string();
                move || hash_password(&password)
            })
            .await?;
        let password_updated_at = current_timestamp_ms()?;
        let auth_version = web_auth::initialize_password_with_bootstrap_token(
            &self.pool,
            &password_hash,
            password_updated_at,
            &bootstrap_token_hash,
        )
        .await
        .map_err(AuthError::Storage)?
        .ok_or(AuthError::BootstrapTokenInvalid)?;
        Ok(AuthState {
            setup_required: false,
            auth_version: auth_version as u64,
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

    pub async fn reset(&self) -> Result<String, AuthError> {
        let jwt_secret = web_auth::load(&self.pool)
            .await
            .map_err(AuthError::Storage)?
            .and_then(|row| row.jwt_secret)
            .unwrap_or_else(jwt::generate_secret);
        let (token, token_hash, expires_at) = self.new_bootstrap_token().await?;
        web_auth::reset(&self.pool, &jwt_secret, &token_hash, expires_at)
            .await
            .map_err(AuthError::Storage)?;
        Ok(token)
    }

    pub async fn issue_bootstrap_token(&self) -> Result<String, AuthError> {
        let existing = validated_record(
            web_auth::load(&self.pool)
                .await
                .map_err(AuthError::Storage)?,
        )?;
        if existing.is_configured() {
            return Err(AuthError::AlreadyInitialized);
        }
        let jwt_secret = existing.jwt_secret.unwrap_or_else(jwt::generate_secret);
        let (token, token_hash, expires_at) = self.new_bootstrap_token().await?;
        let issued =
            web_auth::issue_bootstrap_token(&self.pool, &jwt_secret, &token_hash, expires_at)
                .await
                .map_err(AuthError::Storage)?;
        if !issued {
            return Err(AuthError::AlreadyInitialized);
        }
        Ok(token)
    }

    async fn verify_bootstrap_token(&self, token: &str) -> Result<String, AuthError> {
        if token.len() != 43
            || !token
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err(AuthError::BootstrapTokenInvalid);
        }
        let record = validated_record(
            web_auth::load(&self.pool)
                .await
                .map_err(AuthError::Storage)?,
        )?;
        if record.is_configured() {
            return Err(AuthError::AlreadyInitialized);
        }
        next_auth_version(record.auth_version)?;
        let Some(token_hash) = record.bootstrap_token_hash else {
            return Err(AuthError::BootstrapTokenInvalid);
        };
        let expires_at = record
            .bootstrap_token_expires_at
            .ok_or(AuthError::BootstrapTokenInvalid)?;
        if expires_at <= current_timestamp_ms()? {
            return Err(AuthError::BootstrapTokenInvalid);
        }
        let token = token.to_string();
        let token_hash_for_verify = token_hash.clone();
        let valid = self
            .password_hash_slots
            .run(move || Ok(verify_password_hash(&token, &token_hash_for_verify)))
            .await?;
        if valid {
            Ok(token_hash)
        } else {
            Err(AuthError::BootstrapTokenInvalid)
        }
    }

    async fn new_bootstrap_token(&self) -> Result<(String, String, i64), AuthError> {
        let mut bytes = [0_u8; 32];
        OsRng
            .try_fill_bytes(&mut bytes)
            .map_err(|error| invalid_state(&format!("生成初始化凭据失败：{error}")))?;
        let token = URL_SAFE_NO_PAD.encode(bytes);
        let token_for_hash = token.clone();
        let token_hash = self
            .password_hash_slots
            .run(move || hash_password(&token_for_hash))
            .await?;
        let expires_at = current_timestamp_ms()?
            .checked_add(BOOTSTRAP_TOKEN_TTL_SECONDS * 1000)
            .ok_or_else(|| invalid_state("初始化凭据过期时间超出范围"))?;
        Ok((token, token_hash, expires_at))
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
    password_hash: Option<String>,
    password_updated_at: Option<i64>,
    auth_version: u64,
    jwt_secret: Option<String>,
    bootstrap_token_hash: Option<String>,
    bootstrap_token_expires_at: Option<i64>,
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
            password_hash: None,
            password_updated_at: None,
            auth_version: 0,
            jwt_secret: None,
            bootstrap_token_hash: None,
            bootstrap_token_expires_at: None,
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
        password_hash: row.password_hash,
        password_updated_at: row.password_updated_at,
        auth_version,
        jwt_secret: row.jwt_secret,
        bootstrap_token_hash: row.bootstrap_token_hash,
        bootstrap_token_expires_at: row.bootstrap_token_expires_at,
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
