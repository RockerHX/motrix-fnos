use super::AuthError;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Version};
use rand_core::OsRng;
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[derive(Clone)]
pub(super) struct PasswordHashSlots {
    semaphore: Arc<Semaphore>,
}

impl PasswordHashSlots {
    pub(super) fn new(limit: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(limit)),
        }
    }

    pub(super) async fn run<T, F>(&self, work: F) -> Result<T, AuthError>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, AuthError> + Send + 'static,
    {
        let permit: OwnedSemaphorePermit = Arc::clone(&self.semaphore)
            .try_acquire_owned()
            .map_err(|_| AuthError::PasswordHashBusy)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            work()
        })
        .await
        .map_err(|error| AuthError::InvalidState(format!("密码哈希任务异常：{error}")))?
    }
}

const MIN_PASSWORD_CHARS: usize = 8;
const MAX_PASSWORD_CHARS: usize = 128;
const MAX_PASSWORD_BYTES: usize = 512;

pub(super) fn validate_password(password: &str) -> Result<(), AuthError> {
    let chars = password.chars().count();
    if chars < MIN_PASSWORD_CHARS {
        return Err(AuthError::InvalidPassword(format!(
            "密码至少需要 {MIN_PASSWORD_CHARS} 个字符"
        )));
    }
    if chars > MAX_PASSWORD_CHARS || password.len() > MAX_PASSWORD_BYTES {
        return Err(AuthError::InvalidPassword(format!(
            "密码不能超过 {MAX_PASSWORD_CHARS} 个字符或 {MAX_PASSWORD_BYTES} 字节"
        )));
    }
    Ok(())
}

pub(super) fn hash_password(password: &str) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut OsRng);
    argon2()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| AuthError::InvalidState(format!("生成密码哈希失败：{error}")))
}

pub(super) fn verify_password_hash(password: &str, encoded_hash: &str) -> bool {
    PasswordHash::new(encoded_hash)
        .ok()
        .and_then(|hash| argon2().verify_password(password.as_bytes(), &hash).ok())
        .is_some()
}

fn argon2() -> Argon2<'static> {
    Argon2::new(Algorithm::Argon2id, Version::V0x13, Default::default())
}

#[cfg(test)]
mod tests;
