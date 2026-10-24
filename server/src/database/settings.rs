use serde::de::DeserializeOwned;
use serde::Serialize;
use sqlx::SqlitePool;
use std::time::{SystemTime, UNIX_EPOCH};

pub const DOWNLOAD_PROXY_CONFIG_KEY: &str = "download_proxy";

#[derive(Debug, Clone, Copy, Default, Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BtRuntimeApplyStatus {
    Applied,
    #[default]
    Deferred,
    Blocked,
}

#[derive(Clone, Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StoredDownloadProxyConfig {
    #[serde(default)]
    pub proxy_url: Option<String>,
    pub revision: u64,
    pub updated_at: u64,
    #[serde(default)]
    pub bt_use_proxy: bool,
    #[serde(default)]
    pub bt_runtime_apply: BtRuntimeApplyStatus,
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
    proxy_url: Option<String>,
    bt_use_proxy: Option<bool>,
    initial_apply_status: BtRuntimeApplyStatus,
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
    let Some(current) = current else {
        if proxy_url.is_none() && !bt_use_proxy.unwrap_or(false) {
            transaction
                .commit()
                .await
                .map_err(|error| format!("提交下载代理读取事务失败：{}", error))?;
            return Ok(ReplaceDownloadProxyConfigResult {
                config: StoredDownloadProxyConfig {
                    proxy_url: None,
                    revision: 0,
                    updated_at: 0,
                    bt_use_proxy: false,
                    bt_runtime_apply: initial_apply_status,
                },
                changed: false,
            });
        }
        let config = StoredDownloadProxyConfig {
            proxy_url,
            revision: 1,
            updated_at,
            bt_use_proxy: bt_use_proxy.unwrap_or(false),
            bt_runtime_apply: initial_apply_status,
        };
        write_download_proxy_config(&mut transaction, &config, updated_at).await?;
        transaction
            .commit()
            .await
            .map_err(|error| format!("提交下载代理配置失败：{}", error))?;
        return Ok(ReplaceDownloadProxyConfigResult {
            config,
            changed: true,
        });
    };
    let next_proxy_url = proxy_url.unwrap_or_else(|| current.proxy_url.clone().unwrap_or_default());
    let next_proxy_url = (!next_proxy_url.is_empty()).then_some(next_proxy_url);
    let next_bt_use_proxy = bt_use_proxy.unwrap_or(current.bt_use_proxy);
    if current.proxy_url == next_proxy_url && current.bt_use_proxy == next_bt_use_proxy {
        transaction
            .commit()
            .await
            .map_err(|error| format!("提交下载代理读取事务失败：{}", error))?;
        return Ok(ReplaceDownloadProxyConfigResult {
            config: current,
            changed: false,
        });
    }

    let config = StoredDownloadProxyConfig {
        proxy_url: next_proxy_url,
        revision: current
            .revision
            .checked_add(1)
            .ok_or_else(|| "下载代理配置 revision 已耗尽".to_string())?,
        updated_at,
        bt_use_proxy: next_bt_use_proxy,
        bt_runtime_apply: initial_apply_status,
    };
    write_download_proxy_config(&mut transaction, &config, updated_at).await?;
    transaction
        .commit()
        .await
        .map_err(|error| format!("提交下载代理配置失败：{}", error))?;
    Ok(ReplaceDownloadProxyConfigResult {
        config,
        changed: true,
    })
}

pub async fn set_download_proxy_runtime_apply_status(
    pool: &SqlitePool,
    status: BtRuntimeApplyStatus,
) -> Result<(), String> {
    let Some(mut config) = get_download_proxy_config(pool).await? else {
        return Ok(());
    };
    config.bt_runtime_apply = status;
    let value = serde_json::to_string(&config)
        .map_err(|error| format!("序列化下载代理配置失败：{}", error))?;
    sqlx::query("UPDATE app_config SET value = ? WHERE key = ?")
        .bind(value)
        .bind(DOWNLOAD_PROXY_CONFIG_KEY)
        .execute(pool)
        .await
        .map_err(|error| format!("保存 BT 代理运行状态失败：{}", error))?;
    Ok(())
}

pub async fn delete_download_proxy_config_if_unused(
    pool: &SqlitePool,
) -> Result<DeleteDownloadProxyConfigResult, String> {
    delete_download_proxy_config_for_engine_if_unused(pool, None).await
}

pub(crate) async fn delete_download_proxy_config_for_engine_if_unused(
    pool: &SqlitePool,
    generation: Option<crate::config::aria2::EngineGeneration>,
) -> Result<DeleteDownloadProxyConfigResult, String> {
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| format!("启动清除下载代理事务失败：{}", error))?;
    let value: Option<String> = sqlx::query_scalar("SELECT value FROM app_config WHERE key = ?")
        .bind(DOWNLOAD_PROXY_CONFIG_KEY)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| format!("读取下载代理配置失败：{}", error))?;
    let current = value
        .map(|value| {
            serde_json::from_str::<StoredDownloadProxyConfig>(&value)
                .map_err(|error| format!("解析下载代理配置失败：{}", error))
        })
        .transpose()?;
    let http_in_use: i64 = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM download_tasks WHERE source_type = 'url' AND use_proxy = 1 AND proxy_source = 'profile' AND (? IS NULL OR engine_generation = ?))",
    )
    .bind(generation.map(|generation| generation.as_storage_value()))
    .bind(generation.map(|generation| generation.as_storage_value()))
    .fetch_one(&mut *transaction)
    .await
    .map_err(|error| format!("检查下载代理引用失败：{}", error))?;
    if http_in_use != 0 || current.as_ref().is_some_and(|config| config.bt_use_proxy) {
        transaction
            .commit()
            .await
            .map_err(|error| format!("提交下载代理引用检查失败：{}", error))?;
        return Ok(DeleteDownloadProxyConfigResult::InUse);
    }
    if let Some(mut config) = current.filter(|config| config.proxy_url.is_some()) {
        config.proxy_url = None;
        config.revision = config
            .revision
            .checked_add(1)
            .ok_or_else(|| "下载代理配置 revision 已耗尽".to_string())?;
        config.updated_at = current_timestamp_ms();
        write_download_proxy_config(&mut transaction, &config, config.updated_at).await?;
    }
    transaction
        .commit()
        .await
        .map_err(|error| format!("提交清除下载代理配置失败：{}", error))?;
    Ok(DeleteDownloadProxyConfigResult::Deleted)
}

async fn write_download_proxy_config(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    config: &StoredDownloadProxyConfig,
    updated_at: u64,
) -> Result<(), String> {
    let value = serde_json::to_string(config)
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
    .execute(&mut **transaction)
    .await
    .map_err(|error| format!("保存下载代理配置失败：{}", error))?;
    Ok(())
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
