use sqlx::SqlitePool;

type WebAuthColumns = (
    i64,
    Option<String>,
    Option<i64>,
    i64,
    Option<String>,
    Option<String>,
    Option<i64>,
);

#[derive(Debug, Clone)]
pub(crate) struct WebAuthRow {
    pub enabled: i64,
    pub password_hash: Option<String>,
    pub password_updated_at: Option<i64>,
    pub auth_version: i64,
    pub jwt_secret: Option<String>,
    pub bootstrap_token_hash: Option<String>,
    pub bootstrap_token_expires_at: Option<i64>,
}

pub(crate) async fn load(pool: &SqlitePool) -> Result<Option<WebAuthRow>, String> {
    sqlx::query_as::<_, WebAuthColumns>(
        "SELECT enabled, password_hash, password_updated_at, auth_version, jwt_secret, bootstrap_token_hash, bootstrap_token_expires_at FROM web_auth_config WHERE id = 1",
    )
    .fetch_optional(pool)
    .await
    .map(|row| row.map(web_auth_row))
    .map_err(|error| format!("读取 Web 鉴权配置失败：{error}"))
}

fn web_auth_row(row: WebAuthColumns) -> WebAuthRow {
    WebAuthRow {
        enabled: row.0,
        password_hash: row.1,
        password_updated_at: row.2,
        auth_version: row.3,
        jwt_secret: row.4,
        bootstrap_token_hash: row.5,
        bootstrap_token_expires_at: row.6,
    }
}

pub(crate) async fn initialize_password(
    pool: &SqlitePool,
    password_hash: &str,
    updated_at: i64,
    bootstrap_token_hash: &str,
) -> Result<Option<i64>, String> {
    sqlx::query_scalar(
        "UPDATE web_auth_config SET enabled = 1, password_hash = ?, password_updated_at = ?, auth_version = auth_version + 1, bootstrap_token_hash = NULL, bootstrap_token_expires_at = NULL WHERE id = 1 AND password_hash IS NULL AND password_updated_at IS NULL AND bootstrap_token_hash = ? AND bootstrap_token_expires_at > unixepoch('subsec') * 1000 AND auth_version < 9223372036854775807 RETURNING auth_version",
    )
    .bind(password_hash)
    .bind(updated_at)
    .bind(bootstrap_token_hash)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("初始化 Web 管理密码失败：{error}"))
}

pub(crate) async fn update_password(
    pool: &SqlitePool,
    password_hash: &str,
    updated_at: i64,
    expected_auth_version: i64,
    expected_password_hash: &str,
) -> Result<bool, String> {
    let result = sqlx::query(
        "UPDATE web_auth_config SET enabled = 1, password_hash = ?, password_updated_at = ?, auth_version = auth_version + 1 WHERE id = 1 AND auth_version = ? AND password_hash = ?",
    )
    .bind(password_hash)
    .bind(updated_at)
    .bind(expected_auth_version)
    .bind(expected_password_hash)
    .execute(pool)
    .await
    .map_err(|error| format!("修改 Web 管理密码失败：{error}"))?;
    Ok(result.rows_affected() == 1)
}

pub(crate) async fn issue_bootstrap_token(
    pool: &SqlitePool,
    jwt_secret: &str,
    token_hash: &str,
    expires_at: i64,
) -> Result<bool, String> {
    let result = sqlx::query(
        "INSERT INTO web_auth_config (id, enabled, password_hash, password_updated_at, auth_version, jwt_secret, bootstrap_token_hash, bootstrap_token_expires_at) VALUES (1, 1, NULL, NULL, 1, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET bootstrap_token_hash = excluded.bootstrap_token_hash, bootstrap_token_expires_at = excluded.bootstrap_token_expires_at WHERE web_auth_config.password_hash IS NULL",
    )
    .bind(jwt_secret)
    .bind(token_hash)
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(|error| format!("生成 Web 鉴权初始化凭据失败：{error}"))?;
    Ok(result.rows_affected() == 1)
}

pub(crate) async fn reset(
    pool: &SqlitePool,
    jwt_secret: &str,
    token_hash: &str,
    expires_at: i64,
) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT INTO web_auth_config (id, enabled, password_hash, password_updated_at, auth_version, jwt_secret, bootstrap_token_hash, bootstrap_token_expires_at)
        VALUES (1, 1, NULL, NULL, 1, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            enabled = 1,
            password_hash = NULL,
            password_updated_at = NULL,
            auth_version = web_auth_config.auth_version + 1,
            jwt_secret = COALESCE(NULLIF(web_auth_config.jwt_secret, ''), excluded.jwt_secret),
            bootstrap_token_hash = excluded.bootstrap_token_hash,
            bootstrap_token_expires_at = excluded.bootstrap_token_expires_at
        "#,
    )
    .bind(jwt_secret)
    .bind(token_hash)
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(|error| format!("重置 Web 鉴权配置失败：{error}"))?;
    Ok(())
}
