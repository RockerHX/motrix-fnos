use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::str::FromStr;

pub mod settings;
pub mod tasks;

pub const DATABASE_FILE_NAME: &str = "motrix-fnos.sqlite";

#[derive(Debug, Clone)]
pub struct AppDatabase {
    pub pool: SqlitePool,
    pub path: PathBuf,
}

pub async fn connect_database(path: PathBuf) -> Result<AppDatabase, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("创建数据库目录失败：{}（{}）", parent.display(), error))?;
    }

    let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
        .map_err(|error| format!("创建 SQLite 连接配置失败：{}", error))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .map_err(|error| format!("连接 SQLite 数据库失败：{}", error))?;

    initialize_schema(&pool).await?;

    Ok(AppDatabase { pool, path })
}

async fn initialize_schema(pool: &SqlitePool) -> Result<(), String> {
    for statement in SCHEMA_STATEMENTS {
        sqlx::query(statement)
            .execute(pool)
            .await
            .map_err(|error| format!("初始化 SQLite 数据表失败：{}", error))?;
    }

    migrate_schema(pool).await?;

    Ok(())
}

async fn migrate_schema(pool: &SqlitePool) -> Result<(), String> {
    if download_tasks_column_count(pool, "category").await? == 0 {
        sqlx::query("ALTER TABLE download_tasks ADD COLUMN category TEXT NOT NULL DEFAULT '默认'")
            .execute(pool)
            .await
            .map_err(|error| format!("迁移下载任务分类字段失败：{}", error))?;
    }

    if download_tasks_column_count(pool, "confirmation_required").await? == 0 {
        sqlx::query(
            "ALTER TABLE download_tasks ADD COLUMN confirmation_required INTEGER NOT NULL DEFAULT 0",
        )
        .execute(pool)
        .await
        .map_err(|error| format!("迁移下载任务文件确认字段失败：{}", error))?;
    }

    if download_tasks_column_count(pool, "metadata_torrent_path").await? == 0 {
        sqlx::query("ALTER TABLE download_tasks ADD COLUMN metadata_torrent_path TEXT")
            .execute(pool)
            .await
            .map_err(|error| format!("迁移磁链 metadata 路径字段失败：{}", error))?;
    }

    // 旧版本创建的 UI 偏好表从未承载已上线功能，移除预留接口时同步清理遗留空表。
    sqlx::query("DROP TABLE IF EXISTS ui_preferences")
        .execute(pool)
        .await
        .map_err(|error| format!("清理旧 UI 偏好表失败：{}", error))?;

    Ok(())
}

async fn download_tasks_column_count(pool: &SqlitePool, column: &str) -> Result<i64, String> {
    sqlx::query_scalar("SELECT COUNT(*) FROM pragma_table_info('download_tasks') WHERE name = ?")
        .bind(column)
        .fetch_one(pool)
        .await
        .map_err(|error| format!("检查下载任务字段 {} 失败：{}", column, error))
}

const SCHEMA_STATEMENTS: &[&str] = &[
    r#"
    CREATE TABLE IF NOT EXISTS download_tasks (
        id INTEGER PRIMARY KEY,
        url TEXT NOT NULL,
        file_name TEXT NOT NULL,
        save_dir TEXT NOT NULL,
        category TEXT NOT NULL DEFAULT '默认',
        gid TEXT,
        status TEXT NOT NULL,
        total_length INTEGER NOT NULL DEFAULT 0,
        completed_length INTEGER NOT NULL DEFAULT 0,
        download_speed INTEGER NOT NULL DEFAULT 0,
        error_code TEXT,
        error_message TEXT,
        file_path TEXT,
        confirmation_required INTEGER NOT NULL DEFAULT 0,
        metadata_torrent_path TEXT,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS app_config (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL,
        updated_at INTEGER NOT NULL
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS task_history (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        task_id INTEGER NOT NULL,
        status TEXT NOT NULL,
        message TEXT,
        created_at INTEGER NOT NULL
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS task_errors (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        task_id INTEGER NOT NULL,
        error_code TEXT,
        error_message TEXT NOT NULL,
        created_at INTEGER NOT NULL
    )
    "#,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_database_creates_required_tables() {
        tokio::runtime::Runtime::new()
            .expect("tokio runtime should create")
            .block_on(async {
                let path = std::env::temp_dir().join(format!(
                    "motrix-fnos-db-test-{}.sqlite",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .expect("system time should be valid")
                        .as_millis()
                ));

                let database = connect_database(path.clone())
                    .await
                    .expect("database should connect");

                for table in [
                    "download_tasks",
                    "app_config",
                    "task_history",
                    "task_errors",
                ] {
                    let exists: i64 = sqlx::query_scalar(
                        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
                    )
                    .bind(table)
                    .fetch_one(&database.pool)
                    .await
                    .expect("table lookup should succeed");
                    assert_eq!(exists, 1, "{table} should exist");
                }

                let ui_preferences_exists: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'ui_preferences'",
                )
                .fetch_one(&database.pool)
                .await
                .expect("ui preferences table lookup should succeed");
                assert_eq!(ui_preferences_exists, 0);

                for column in ["category", "confirmation_required", "metadata_torrent_path"] {
                    let column_count: i64 = sqlx::query_scalar(
                        "SELECT COUNT(*) FROM pragma_table_info('download_tasks') WHERE name = ?",
                    )
                    .bind(column)
                    .fetch_one(&database.pool)
                    .await
                    .expect("column lookup should succeed");
                    assert_eq!(column_count, 1, "download_tasks.{column} should exist");
                }

                database.pool.close().await;
                let _ = std::fs::remove_file(path);
            });
    }

    #[test]
    fn connect_database_removes_legacy_ui_preferences_table() {
        tokio::runtime::Runtime::new()
            .expect("tokio runtime should create")
            .block_on(async {
                let path = std::env::temp_dir().join(format!(
                    "motrix-fnos-ui-preferences-migrate-test-{}.sqlite",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .expect("system time should be valid")
                        .as_nanos()
                ));
                let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
                    .expect("sqlite options should build")
                    .create_if_missing(true);
                let pool = SqlitePoolOptions::new()
                    .max_connections(1)
                    .connect_with(options)
                    .await
                    .expect("legacy db should connect");
                sqlx::query("CREATE TABLE ui_preferences (key TEXT PRIMARY KEY, value TEXT NOT NULL, updated_at INTEGER NOT NULL)")
                    .execute(&pool)
                    .await
                    .expect("legacy ui preferences table should create");
                pool.close().await;

                let database = connect_database(path.clone())
                    .await
                    .expect("database should connect and migrate");
                let exists: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'ui_preferences'",
                )
                .fetch_one(&database.pool)
                .await
                .expect("ui preferences table lookup should succeed");
                assert_eq!(exists, 0);

                database.pool.close().await;
                let _ = std::fs::remove_file(path);
            });
    }

    #[test]
    fn connect_database_migrates_existing_download_tasks_category() {
        tokio::runtime::Runtime::new()
            .expect("tokio runtime should create")
            .block_on(async {
                let path = std::env::temp_dir().join(format!(
                    "motrix-fnos-db-migrate-test-{}.sqlite",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .expect("system time should be valid")
                        .as_millis()
                ));

                {
                    let options =
                        SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
                            .expect("sqlite options should build")
                            .create_if_missing(true);
                    let pool = SqlitePoolOptions::new()
                        .max_connections(1)
                        .connect_with(options)
                        .await
                        .expect("legacy db should connect");
                    sqlx::query(
                        r#"
                        CREATE TABLE download_tasks (
                            id INTEGER PRIMARY KEY,
                            url TEXT NOT NULL,
                            file_name TEXT NOT NULL,
                            save_dir TEXT NOT NULL,
                            gid TEXT,
                            status TEXT NOT NULL,
                            total_length INTEGER NOT NULL DEFAULT 0,
                            completed_length INTEGER NOT NULL DEFAULT 0,
                            download_speed INTEGER NOT NULL DEFAULT 0,
                            error_code TEXT,
                            error_message TEXT,
                            file_path TEXT,
                            created_at INTEGER NOT NULL,
                            updated_at INTEGER NOT NULL
                        )
                        "#,
                    )
                    .execute(&pool)
                    .await
                    .expect("legacy table should create");
                    pool.close().await;
                }

                let database = connect_database(path.clone())
                    .await
                    .expect("database should connect and migrate");
                for column in ["category", "confirmation_required", "metadata_torrent_path"] {
                    let column_count: i64 = sqlx::query_scalar(
                        "SELECT COUNT(*) FROM pragma_table_info('download_tasks') WHERE name = ?",
                    )
                    .bind(column)
                    .fetch_one(&database.pool)
                    .await
                    .expect("column lookup should succeed");
                    assert_eq!(
                        column_count, 1,
                        "download_tasks.{column} should be migrated"
                    );
                }

                database.pool.close().await;
                let _ = std::fs::remove_file(path);
            });
    }
}
