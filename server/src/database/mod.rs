use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::str::FromStr;

pub mod settings;
pub mod tasks;
pub(crate) mod web_auth;

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
    if download_tasks_column_count(pool, "source_type").await? == 0 {
        sqlx::query(
            "ALTER TABLE download_tasks ADD COLUMN source_type TEXT NOT NULL DEFAULT 'url'",
        )
        .execute(pool)
        .await
        .map_err(|error| format!("迁移下载任务来源字段失败：{}", error))?;
        sqlx::query(
            r#"
            UPDATE download_tasks
            SET source_type = CASE
                WHEN LOWER(url) LIKE 'magnet:?%' THEN 'magnet'
                WHEN LOWER(url) LIKE 'torrent:%' THEN 'torrent'
                ELSE 'url'
            END
            "#,
        )
        .execute(pool)
        .await
        .map_err(|error| format!("回填下载任务来源字段失败：{}", error))?;
    }

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

    if download_tasks_column_count(pool, "files_deleted").await? == 0 {
        sqlx::query(
            "ALTER TABLE download_tasks ADD COLUMN files_deleted INTEGER NOT NULL DEFAULT 0",
        )
        .execute(pool)
        .await
        .map_err(|error| format!("迁移任务本地文件删除标记失败：{}", error))?;
    }

    if download_tasks_column_count(pool, "selected_file_indexes").await? == 0 {
        sqlx::query(
            "ALTER TABLE download_tasks ADD COLUMN selected_file_indexes TEXT NOT NULL DEFAULT '[]'",
        )
        .execute(pool)
        .await
        .map_err(|error| format!("迁移任务文件选择字段失败：{}", error))?;
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
        source_type TEXT NOT NULL DEFAULT 'url',
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
        files_deleted INTEGER NOT NULL DEFAULT 0,
        selected_file_indexes TEXT NOT NULL DEFAULT '[]',
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
    r#"
    CREATE TABLE IF NOT EXISTS web_auth_config (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
        password_hash TEXT,
        password_updated_at INTEGER,
        auth_version INTEGER NOT NULL CHECK (auth_version > 0)
    )
    "#,
];

#[cfg(test)]
mod tests;
