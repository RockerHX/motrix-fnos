use super::*;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

#[test]
fn round_trip_and_version_validation() {
    let secret = generate_secret();
    assert_eq!(URL_SAFE_NO_PAD.decode(&secret).unwrap().len(), 32);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let token = issue(&secret, 3, now).expect("token should issue");
    let claims = validate(&secret, &token, 3).expect("token should validate");
    assert_eq!(claims.sub, JWT_SUBJECT);
    assert_eq!(claims.role, JWT_ROLE);
    assert_eq!(claims.exp.saturating_sub(claims.iat), JWT_LIFETIME_SECONDS);
    assert_eq!(
        validate(&secret, &token, 4),
        Err(JwtValidationFailure::AuthVersionMismatch)
    );
}

#[test]
fn rejects_tampering_and_malformed_tokens() {
    let secret = generate_secret();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let token = issue(&secret, 1, now).expect("token should issue");
    let (prefix, signature) = token
        .rsplit_once('.')
        .expect("JWT should contain a signature");
    let mut signature = signature.as_bytes().to_vec();
    signature[0] = if signature[0] == b'a' { b'b' } else { b'a' };
    let tampered = format!(
        "{prefix}.{}",
        String::from_utf8(signature).expect("signature should remain UTF-8")
    );
    assert_eq!(
        validate(&secret, &tampered, 1),
        Err(JwtValidationFailure::Invalid)
    );
    assert_eq!(
        validate(&secret, "bad", 1),
        Err(JwtValidationFailure::Malformed)
    );
}

#[test]
fn rejects_expired_tokens() {
    let secret = generate_secret();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let token = issue(&secret, 1, now).expect("token should issue");
    assert!(validate(&secret, &token, 1).is_ok());
    let expired = jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
        &Claims {
            sub: JWT_SUBJECT.to_string(),
            role: JWT_ROLE.to_string(),
            iat: now.saturating_sub(100),
            exp: now.saturating_sub(1),
            auth_version: 1,
        },
        &jsonwebtoken::EncodingKey::from_secret(&URL_SAFE_NO_PAD.decode(&secret).unwrap()),
    )
    .unwrap();
    assert_eq!(
        validate(&secret, &expired, 1),
        Err(JwtValidationFailure::Expired)
    );
}

#[test]
fn rejects_tokens_without_the_admin_role() {
    let secret = generate_secret();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let token = sign_claims(
        &secret,
        Claims {
            sub: JWT_SUBJECT.to_string(),
            role: "viewer".to_string(),
            iat: now,
            exp: now.saturating_add(JWT_LIFETIME_SECONDS),
            auth_version: 1,
        },
    );

    assert_eq!(
        validate(&secret, &token, 1),
        Err(JwtValidationFailure::InsufficientPrivileges)
    );
}

fn sign_claims(secret: &str, claims: Claims) -> String {
    jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(&URL_SAFE_NO_PAD.decode(secret).unwrap()),
    )
    .unwrap()
}
