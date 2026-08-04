use serde::de::DeserializeOwned;
use serde::Serialize;
use sqlx::SqlitePool;
use std::time::{SystemTime, UNIX_EPOCH};

pub const DOWNLOAD_PROXY_CONFIG_KEY: &str = "download_proxy";

#[derive(Clone, Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StoredDownloadProxyConfig {
    pub proxy_url: String,
    pub revision: u64,
    pub updated_at: u64,
}

pub async fn get_download_proxy_config(
    pool: &SqlitePool,
) -> Result<Option<StoredDownloadProxyConfig>, String> {
    get_app_config_value(pool, DOWNLOAD_PROXY_CONFIG_KEY).await
}

pub struct ReplaceDownloadProxyConfigResult {
    pub config: StoredDownloadProxyConfig,
    pub changed: bool,
}

pub enum DeleteDownloadProxyConfigResult {
    Deleted,
    InUse,
}

pub async fn replace_download_proxy_config(
    pool: &SqlitePool,
    proxy_url: String,
    updated_at: u64,
) -> Result<ReplaceDownloadProxyConfigResult, String> {
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| format!("启动保存下载代理事务失败：{}", error))?;
    let current: Option<String> = sqlx::query_scalar("SELECT value FROM app_config WHERE key = ?")
        .bind(DOWNLOAD_PROXY_CONFIG_KEY)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| format!("读取下载代理配置失败：{}", error))?;
    let current = current
        .map(|value| {
            serde_json::from_str::<StoredDownloadProxyConfig>(&value)
                .map_err(|error| format!("解析下载代理配置失败：{}", error))
        })
        .transpose()?;
    if current
        .as_ref()
        .is_some_and(|current| current.proxy_url == proxy_url)
    {
        transaction
            .commit()
            .await
            .map_err(|error| format!("提交下载代理读取事务失败：{}", error))?;
        return Ok(ReplaceDownloadProxyConfigResult {
            config: current.expect("checked current proxy config"),
            changed: false,
        });
    }

    let config = StoredDownloadProxyConfig {
        proxy_url,
        revision: current
            .map(|current| current.revision)
            .unwrap_or_default()
            .checked_add(1)
            .ok_or_else(|| "下载代理配置 revision 已耗尽".to_string())?,
        updated_at,
    };
    let value = serde_json::to_string(&config)
        .map_err(|error| format!("序列化下载代理配置失败：{}", error))?;
    sqlx::query(
        r#"
        INSERT INTO app_config (key, value, updated_at)
        VALUES (?, ?, ?)
        ON CONFLICT(key) DO UPDATE SET
            value = excluded.value,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(DOWNLOAD_PROXY_CONFIG_KEY)
    .bind(value)
    .bind(i64::try_from(updated_at).unwrap_or(i64::MAX))
    .execute(&mut *transaction)
    .await
    .map_err(|error| format!("保存下载代理配置失败：{}", error))?;
    transaction
        .commit()
        .await
        .map_err(|error| format!("提交下载代理配置失败：{}", error))?;
    Ok(ReplaceDownloadProxyConfigResult {
        config,
        changed: true,
    })
}

pub async fn delete_download_proxy_config_if_unused(
    pool: &SqlitePool,
) -> Result<DeleteDownloadProxyConfigResult, String> {
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| format!("启动清除下载代理事务失败：{}", error))?;
    let in_use: i64 = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM download_tasks WHERE use_proxy = 1 AND proxy_source = 'profile')",
    )
    .fetch_one(&mut *transaction)
    .await
    .map_err(|error| format!("检查下载代理引用失败：{}", error))?;
    if in_use != 0 {
        transaction
            .commit()
            .await
            .map_err(|error| format!("提交下载代理引用检查失败：{}", error))?;
        return Ok(DeleteDownloadProxyConfigResult::InUse);
    }
    sqlx::query("DELETE FROM app_config WHERE key = ?")
        .bind(DOWNLOAD_PROXY_CONFIG_KEY)
        .execute(&mut *transaction)
        .await
        .map_err(|error| format!("清除下载代理配置失败：{}", error))?;
    transaction
        .commit()
        .await
        .map_err(|error| format!("提交清除下载代理配置失败：{}", error))?;
    Ok(DeleteDownloadProxyConfigResult::Deleted)
}

pub async fn get_app_config_value<T>(pool: &SqlitePool, key: &str) -> Result<Option<T>, String>
where
    T: DeserializeOwned,
{
    get_json_value(pool, "app_config", key).await
}

pub async fn set_app_config_value<T>(pool: &SqlitePool, key: &str, value: &T) -> Result<(), String>
where
    T: Serialize,
{
    set_json_value(pool, "app_config", key, value).await
}

async fn get_json_value<T>(pool: &SqlitePool, table: &str, key: &str) -> Result<Option<T>, String>
where
    T: DeserializeOwned,
{
    let value: Option<String> =
        sqlx::query_scalar(&format!("SELECT value FROM {table} WHERE key = ?"))
            .bind(key)
            .fetch_optional(pool)
            .await
            .map_err(|error| format!("读取配置失败：{}", error))?;

    value
        .map(|value| {
            serde_json::from_str(&value).map_err(|error| format!("解析配置失败：{}", error))
        })
        .transpose()
}

async fn set_json_value<T>(
    pool: &SqlitePool,
    table: &str,
    key: &str,
    value: &T,
) -> Result<(), String>
where
    T: Serialize,
{
    let value =
        serde_json::to_string(value).map_err(|error| format!("序列化配置失败：{}", error))?;
    sqlx::query(&format!(
        r#"
        INSERT INTO {table} (key, value, updated_at)
        VALUES (?, ?, ?)
        ON CONFLICT(key) DO UPDATE SET
            value = excluded.value,
            updated_at = excluded.updated_at
        "#,
    ))
    .bind(key)
    .bind(value)
    .bind(current_timestamp_ms() as i64)
    .execute(pool)
    .await
    .map_err(|error| format!("保存配置失败：{}", error))?;

    Ok(())
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
