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
fn auth_service_supports_setup_password_change_and_reset() {
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
        assert_eq!(state.auth_version, 1);
        let original_token = service
            .issue_admin_token(&state)
            .await
            .expect("token should issue");
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

        service.reset().await.expect("reset should pass");
        let reset = service.state().await.expect("reset state should load");
        assert!(reset.setup_required);
        assert_eq!(reset.auth_version, 3);
        assert_eq!(
            service
                .validate_admin_token(&original_token, reset.auth_version)
                .await,
            Err(JwtValidationFailure::AuthVersionMismatch)
        );
        cleanup(service, path).await;
    });
}

#[test]
fn upgrade_reenables_protection_without_changing_the_password() {
    test_runtime().block_on(async {
        let (service, path) = test_service("force-protection").await;
        let configured = service
            .setup(VALID_PASSWORD)
            .await
            .expect("setup should pass");
        let token = service
            .issue_admin_token(&configured)
            .await
            .expect("token should issue");
        let password_hash: String =
            sqlx::query_scalar("SELECT password_hash FROM web_auth_config WHERE id = 1")
                .fetch_one(&service.pool)
                .await
                .expect("password hash should load");
        sqlx::query("UPDATE web_auth_config SET enabled = 0 WHERE id = 1")
            .execute(&service.pool)
            .await
            .expect("legacy protection state should persist");
        sqlx::query("DELETE FROM schema_migrations WHERE version = 6")
            .execute(&service.pool)
            .await
            .expect("migration record should clear");
        service.pool.close().await;

        let database = connect_database(path.clone())
            .await
            .expect("database should upgrade");
        let reopened = AuthService::new(database.pool);
        let enabled: i64 = sqlx::query_scalar("SELECT enabled FROM web_auth_config WHERE id = 1")
            .fetch_one(&reopened.pool)
            .await
            .expect("protection state should load");
        let restored_hash: String =
            sqlx::query_scalar("SELECT password_hash FROM web_auth_config WHERE id = 1")
                .fetch_one(&reopened.pool)
                .await
                .expect("password hash should load");
        let upgraded = reopened.state().await.expect("state should load");

        assert_eq!(enabled, 1);
        assert_eq!(restored_hash, password_hash);
        assert_eq!(upgraded.auth_version, configured.auth_version + 1);
        assert!(reopened.verify_password(VALID_PASSWORD).await.is_ok());
        assert_eq!(
            reopened
                .validate_admin_token(&token, upgraded.auth_version)
                .await,
            Err(JwtValidationFailure::AuthVersionMismatch)
        );
        cleanup(reopened, path).await;
    });
}

#[test]
fn issued_tokens_remain_verifiable_after_database_reopen() {
    test_runtime().block_on(async {
        let (service, path) = test_service("jwt-reopen").await;
        let state = service
            .setup(VALID_PASSWORD)
            .await
            .expect("setup should pass");
        let token = service
            .issue_admin_token(&state)
            .await
            .expect("token should issue");
        service.pool.close().await;

        let database = connect_database(path.clone())
            .await
            .expect("database should reopen");
        let reopened = AuthService::new(database.pool);
        let reopened_state = reopened.state().await.expect("state should load");
        assert!(reopened
            .validate_admin_token(&token, reopened_state.auth_version)
            .await
            .is_ok());
        cleanup(reopened, path).await;
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
