use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand_core::{OsRng, RngCore};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use subtle::ConstantTimeEq;

pub const SESSION_COOKIE_NAME: &str = "motrix_web_session";
const SESSION_ABSOLUTE_LIFETIME_MS: u64 = 12 * 60 * 60 * 1_000;
const SESSION_IDLE_TIMEOUT_MS: u64 = 30 * 60 * 1_000;
const TOKEN_BYTES: usize = 32;

type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    Admin,
    AnonymousManagement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedSession {
    pub id: String,
    pub csrf_token: String,
    pub kind: SessionKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSession {
    pub id: String,
    pub csrf_token: String,
    pub kind: SessionKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    StoreUnavailable,
}

#[derive(Clone)]
pub struct SessionStore {
    sessions: Arc<Mutex<HashMap<String, Session>>>,
    clock: Clock,
}

struct Session {
    kind: SessionKind,
    csrf_token: [u8; TOKEN_BYTES],
    auth_version: u64,
    issued_at_ms: u64,
    last_active_at_ms: u64,
}

impl SessionStore {
    pub fn new() -> Self {
        Self::with_clock(Arc::new(current_timestamp_ms))
    }

    pub fn create(
        &self,
        kind: SessionKind,
        auth_version: u64,
    ) -> Result<CreatedSession, SessionError> {
        let now = (self.clock)();
        let mut id_bytes = [0_u8; TOKEN_BYTES];
        let mut csrf_bytes = [0_u8; TOKEN_BYTES];
        OsRng.fill_bytes(&mut id_bytes);
        OsRng.fill_bytes(&mut csrf_bytes);
        let id = URL_SAFE_NO_PAD.encode(id_bytes);
        let csrf_token = URL_SAFE_NO_PAD.encode(csrf_bytes);
        self.sessions
            .lock()
            .map_err(|_| SessionError::StoreUnavailable)?
            .insert(
                id.clone(),
                Session {
                    kind,
                    csrf_token: csrf_bytes,
                    auth_version,
                    issued_at_ms: now,
                    last_active_at_ms: now,
                },
            );
        Ok(CreatedSession {
            id,
            csrf_token,
            kind,
        })
    }

    pub fn validate(
        &self,
        id: &str,
        auth_version: u64,
    ) -> Result<Option<ValidatedSession>, SessionError> {
        let now = (self.clock)();
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| SessionError::StoreUnavailable)?;
        sessions.retain(|_, session| !is_expired(session, now));
        let Some(session) = sessions.get_mut(id) else {
            return Ok(None);
        };
        if session.auth_version != auth_version {
            sessions.remove(id);
            return Ok(None);
        }
        session.last_active_at_ms = now;
        Ok(Some(ValidatedSession {
            id: id.to_string(),
            csrf_token: URL_SAFE_NO_PAD.encode(session.csrf_token),
            kind: session.kind,
        }))
    }

    pub fn validate_csrf(
        &self,
        id: &str,
        auth_version: u64,
        candidate: &str,
    ) -> Result<bool, SessionError> {
        let Some(session) = self.validate(id, auth_version)? else {
            return Ok(false);
        };
        let Ok(candidate) = URL_SAFE_NO_PAD.decode(candidate) else {
            return Ok(false);
        };
        let Ok(candidate) = <[u8; TOKEN_BYTES]>::try_from(candidate.as_slice()) else {
            return Ok(false);
        };
        let expected = URL_SAFE_NO_PAD
            .decode(session.csrf_token)
            .ok()
            .and_then(|value| <[u8; TOKEN_BYTES]>::try_from(value.as_slice()).ok());
        Ok(expected
            .map(|expected| bool::from(expected.ct_eq(&candidate)))
            .unwrap_or(false))
    }

    pub fn revoke(&self, id: &str) -> Result<(), SessionError> {
        self.sessions
            .lock()
            .map_err(|_| SessionError::StoreUnavailable)?
            .remove(id);
        Ok(())
    }

    pub fn revoke_all(&self) -> Result<(), SessionError> {
        self.sessions
            .lock()
            .map_err(|_| SessionError::StoreUnavailable)?
            .clear();
        Ok(())
    }

    fn with_clock(clock: Clock) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            clock,
        }
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

pub fn session_cookie(session_id: &str) -> String {
    format!("{SESSION_COOKIE_NAME}={session_id}; HttpOnly; SameSite=Strict; Path=/; Max-Age=43200")
}

pub fn clear_session_cookie() -> String {
    format!("{SESSION_COOKIE_NAME}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0")
}

fn is_expired(session: &Session, now: u64) -> bool {
    now.saturating_sub(session.issued_at_ms) >= SESSION_ABSOLUTE_LIFETIME_MS
        || now.saturating_sub(session.last_active_at_ms) >= SESSION_IDLE_TIMEOUT_MS
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
