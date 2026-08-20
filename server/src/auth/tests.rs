use super::*;
use crate::database::connect_database;

const VALID_PASSWORD: &str = "correct horse battery";

#[test]
fn password_policy_counts_unicode_without_trimming() {
    assert!(password::validate_password("12345678").is_ok());
    assert!(password::validate_password("十二个字符密码安全测试甲乙").is_ok());
    assert!(password::validate_password("  leading spaces are kept").is_ok());
    assert!(matches!(
        password::validate_password("1234567"),
        Err(AuthError::InvalidPassword(_))
    ));
    assert!(matches!(
        password::validate_password(&"界".repeat(129)),
        Err(AuthError::InvalidPassword(_))
    ));
    assert!(password::validate_password(&"😀".repeat(128)).is_ok());
}

#[test]
fn password_hash_uses_argon2id_and_random_salts() {
    let first = password::hash_password(VALID_PASSWORD).expect("hash should create");
    let second = password::hash_password(VALID_PASSWORD).expect("hash should create");
    assert_ne!(first, second);
    assert!(first.starts_with("$argon2id$v=19$"));
    assert!(password::verify_password_hash(VALID_PASSWORD, &first));
    assert!(!password::verify_password_hash(
        "incorrect password",
        &first
    ));
    assert!(!first.contains(VALID_PASSWORD));
}

#[test]
fn auth_service_supports_setup_change_protection_and_reset() {
    test_runtime().block_on(async {
        let (service, path) = test_service("lifecycle").await;
        assert!(
            service
                .state()
                .await
                .expect("state should load")
                .setup_required
        );

        let state = service
            .setup(VALID_PASSWORD)
            .await
            .expect("setup should pass");
        assert!(!state.setup_required);
        assert!(state.enabled);
        assert_eq!(state.auth_version, 1);
        assert!(service.verify_password(VALID_PASSWORD).await.is_ok());
        assert_eq!(
            service.verify_password("incorrect password").await,
            Err(AuthError::InvalidCredentials)
        );

        let changed = service
            .change_password(VALID_PASSWORD, "replacement password")
            .await
            .expect("password should change");
        assert_eq!(changed.auth_version, 2);
        assert!(service
            .verify_password("replacement password")
            .await
            .is_ok());

        let disabled = service
            .set_protection(false, "replacement password")
            .await
            .expect("protection should change");
        assert!(!disabled.enabled);
        assert_eq!(disabled.auth_version, 3);

        service.reset().await.expect("reset should pass");
        let reset = service.state().await.expect("reset state should load");
        assert!(reset.setup_required);
        assert!(reset.enabled);
        assert_eq!(reset.auth_version, 4);
        cleanup(service, path).await;
    });
}

#[test]
fn concurrent_setup_allows_only_one_password() {
    test_runtime().block_on(async {
        let (service, path) = test_service("concurrent").await;
        let first = service.clone();
        let second = service.clone();
        let (first, second) = tokio::join!(
            first.setup(VALID_PASSWORD),
            second.setup("another secure password")
        );
        assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
        assert!(matches!(first, Ok(_) | Err(AuthError::AlreadyInitialized)));
        assert!(matches!(second, Ok(_) | Err(AuthError::AlreadyInitialized)));
        cleanup(service, path).await;
    });
}

#[test]
fn corrupt_auth_rows_fail_closed() {
    test_runtime().block_on(async {
        let (service, path) = test_service("corrupt").await;
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&service.pool)
            .await
            .expect("pragma should apply");
        sqlx::query("INSERT INTO web_auth_config (id, enabled, password_hash, password_updated_at, auth_version) VALUES (1, 1, 'broken', NULL, 1)")
            .execute(&service.pool)
            .await
            .expect("corrupt row should insert");
        assert!(matches!(service.state().await, Err(AuthError::InvalidState(_))));
        cleanup(service, path).await;
    });
}

fn test_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().expect("runtime should create")
}

async fn test_service(name: &str) -> (AuthService, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!(
        "motrix-fnos-auth-{name}-{}.sqlite",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time should be valid")
            .as_nanos()
    ));
    let database = connect_database(path.clone())
        .await
        .expect("database should connect");
    (AuthService::new(database.pool), path)
}

async fn cleanup(service: AuthService, path: std::path::PathBuf) {
    service.pool.close().await;
    let _ = std::fs::remove_file(path);
}
