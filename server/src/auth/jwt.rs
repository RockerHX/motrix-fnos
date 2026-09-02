use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};

pub const JWT_SUBJECT: &str = "admin";
pub const JWT_ROLE: &str = "admin";
pub const JWT_LIFETIME_SECONDS: u64 = 12 * 60 * 60;
const JWT_SECRET_BYTES: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Claims {
    pub sub: String,
    pub role: String,
    pub iat: u64,
    pub exp: u64,
    pub auth_version: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JwtValidationFailure {
    Malformed,
    Invalid,
    Expired,
    AuthVersionMismatch,
    InsufficientPrivileges,
}

pub fn generate_secret() -> String {
    let mut bytes = [0_u8; JWT_SECRET_BYTES];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn issue(secret: &str, auth_version: u64, now_seconds: u64) -> Result<String, String> {
    let key = encoding_key(secret)?;
    let claims = Claims {
        sub: JWT_SUBJECT.to_string(),
        role: JWT_ROLE.to_string(),
        iat: now_seconds,
        exp: now_seconds.saturating_add(JWT_LIFETIME_SECONDS),
        auth_version,
    };
    encode(&Header::new(Algorithm::HS256), &claims, &key)
        .map_err(|error| format!("签发 Web 管理 JWT 失败：{error}"))
}

pub fn validate(
    secret: &str,
    token: &str,
    auth_version: u64,
) -> Result<Claims, JwtValidationFailure> {
    if token.split('.').count() != 3 || token.is_empty() {
        return Err(JwtValidationFailure::Malformed);
    }
    let key = decoding_key(secret).map_err(|_| JwtValidationFailure::Invalid)?;
    let mut validation = Validation::new(Algorithm::HS256);
    validation.leeway = 0;
    let data = decode::<Claims>(token, &key, &validation).map_err(classify_decode_failure)?;
    let claims = data.claims;
    if claims.sub != JWT_SUBJECT || claims.role != JWT_ROLE {
        return Err(JwtValidationFailure::InsufficientPrivileges);
    }
    if claims.auth_version != auth_version {
        return Err(JwtValidationFailure::AuthVersionMismatch);
    }
    Ok(claims)
}

fn classify_decode_failure(error: jsonwebtoken::errors::Error) -> JwtValidationFailure {
    use jsonwebtoken::errors::ErrorKind;

    match error.kind() {
        ErrorKind::ExpiredSignature => JwtValidationFailure::Expired,
        ErrorKind::MissingRequiredClaim(_)
        | ErrorKind::InvalidClaimFormat(_)
        | ErrorKind::Base64(_)
        | ErrorKind::Json(_)
        | ErrorKind::Utf8(_) => JwtValidationFailure::Malformed,
        _ => JwtValidationFailure::Invalid,
    }
}

fn encoding_key(secret: &str) -> Result<EncodingKey, String> {
    let bytes = decode_secret(secret)?;
    Ok(EncodingKey::from_secret(&bytes))
}

fn decoding_key(secret: &str) -> Result<DecodingKey, String> {
    let bytes = decode_secret(secret)?;
    Ok(DecodingKey::from_secret(&bytes))
}

fn decode_secret(secret: &str) -> Result<Vec<u8>, String> {
    let bytes = URL_SAFE_NO_PAD
        .decode(secret)
        .map_err(|error| format!("JWT 密钥格式非法：{error}"))?;
    if bytes.len() != JWT_SECRET_BYTES {
        return Err("JWT 密钥长度非法".to_string());
    }
    Ok(bytes)
}

fn current_timestamp_seconds() -> Result<u64, String> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| format!("系统时间非法：{error}"))
}

pub fn issue_now(secret: &str, auth_version: u64) -> Result<String, String> {
    issue(secret, auth_version, current_timestamp_seconds()?)
}

#[cfg(test)]
mod tests;
