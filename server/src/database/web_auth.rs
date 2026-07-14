use sqlx::{Sqlite, SqlitePool, Transaction};

#[derive(Debug, Clone)]
pub(crate) struct WebAuthRow {
    pub enabled: i64,
    pub password_hash: Option<String>,
    pub password_updated_at: Option<i64>,
    pub auth_version: i64,
}

pub(crate) async fn load(pool: &SqlitePool) -> Result<Option<WebAuthRow>, String> {
    sqlx::query_as::<_, (i64, Option<String>, Option<i64>, i64)>(
        "SELECT enabled, password_hash, password_updated_at, auth_version FROM web_auth_config WHERE id = 1",
    )
    .fetch_optional(pool)
    .await
    .map(|row| row.map(web_auth_row))
    .map_err(|error| format!("读取 Web 鉴权配置失败：{error}"))
}

pub(crate) async fn load_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
) -> Result<Option<WebAuthRow>, String> {
    sqlx::query_as::<_, (i64, Option<String>, Option<i64>, i64)>(
        "SELECT enabled, password_hash, password_updated_at, auth_version FROM web_auth_config WHERE id = 1",
    )
    .fetch_optional(&mut **transaction)
    .await
    .map(|row| row.map(web_auth_row))
    .map_err(|error| format!("读取 Web 鉴权配置失败：{error}"))
}

fn web_auth_row(row: (i64, Option<String>, Option<i64>, i64)) -> WebAuthRow {
    WebAuthRow {
        enabled: row.0,
        password_hash: row.1,
        password_updated_at: row.2,
        auth_version: row.3,
    }
}

pub(crate) async fn initialize_password(
    pool: &SqlitePool,
    password_hash: &str,
    updated_at: i64,
    has_reset_row: bool,
) -> Result<bool, String> {
    let result = if has_reset_row {
        sqlx::query(
            "UPDATE web_auth_config SET enabled = 1, password_hash = ?, password_updated_at = ?, auth_version = auth_version + 1 WHERE id = 1 AND password_hash IS NULL AND password_updated_at IS NULL",
        )
        .bind(password_hash)
        .bind(updated_at)
        .execute(pool)
        .await
    } else {
        sqlx::query(
            "INSERT OR IGNORE INTO web_auth_config (id, enabled, password_hash, password_updated_at, auth_version) VALUES (1, 1, ?, ?, 1)",
        )
        .bind(password_hash)
        .bind(updated_at)
        .execute(pool)
        .await
    }
    .map_err(|error| format!("初始化 Web 管理密码失败：{error}"))?;

    Ok(result.rows_affected() == 1)
}

pub(crate) async fn update_password(
    transaction: &mut Transaction<'_, Sqlite>,
    password_hash: &str,
    updated_at: i64,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE web_auth_config SET password_hash = ?, password_updated_at = ?, auth_version = auth_version + 1 WHERE id = 1",
    )
    .bind(password_hash)
    .bind(updated_at)
    .execute(&mut **transaction)
    .await
    .map_err(|error| format!("修改 Web 管理密码失败：{error}"))?;
    Ok(())
}

pub(crate) async fn update_protection(
    transaction: &mut Transaction<'_, Sqlite>,
    enabled: bool,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE web_auth_config SET enabled = ?, auth_version = auth_version + 1 WHERE id = 1",
    )
    .bind(i64::from(enabled))
    .execute(&mut **transaction)
    .await
    .map_err(|error| format!("修改 Web 管理保护状态失败：{error}"))?;
    Ok(())
}

pub(crate) async fn reset(pool: &SqlitePool) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT INTO web_auth_config (id, enabled, password_hash, password_updated_at, auth_version)
        VALUES (1, 1, NULL, NULL, 1)
        ON CONFLICT(id) DO UPDATE SET
            enabled = 1,
            password_hash = NULL,
            password_updated_at = NULL,
            auth_version = web_auth_config.auth_version + 1
        "#,
    )
    .execute(pool)
    .await
    .map_err(|error| format!("重置 Web 鉴权配置失败：{error}"))?;
    Ok(())
}
