use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

pub mod maintenance;
pub mod settings;
pub mod task_operations;
pub mod tasks;
pub(crate) mod web_auth;

pub const DATABASE_FILE_NAME: &str = "motrix-fnos.sqlite";

#[derive(Debug, Clone)]
pub struct AppDatabase {
    pub pool: SqlitePool,
    pub path: PathBuf,
}

pub async fn check_integrity(path: PathBuf) -> Result<(), String> {
    let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
        .map_err(|error| format!("创建 SQLite 完整性检查配置失败：{}", error))?
        .read_only(true)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|error| format!("打开 SQLite 数据库失败：{}（{}）", path.display(), error))?;

    let result: String = sqlx::query_scalar("PRAGMA quick_check")
        .fetch_one(&pool)
        .await
        .map_err(|error| format!("执行 SQLite 完整性检查失败：{}", error))?;
    pool.close().await;

    if result.eq_ignore_ascii_case("ok") {
        Ok(())
    } else {
        Err(format!("SQLite 完整性检查失败：{}", result))
    }
}

pub async fn backup_database(source: PathBuf, output: PathBuf) -> Result<(), String> {
    let output = normalize_backup_output(&source, &output)?;
    let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", source.display()))
        .map_err(|error| format!("创建 SQLite 备份配置失败：{}", error))?
        .create_if_missing(false)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|error| format!("打开 SQLite 数据库失败：{}（{}）", source.display(), error))?;

    let output_string = output.to_string_lossy().into_owned();
    let vacuum_result = sqlx::query("VACUUM INTO ?")
        .bind(output_string)
        .execute(&pool)
        .await;
    pool.close().await;

    if let Err(error) = vacuum_result {
        let _ = std::fs::remove_file(&output);
        return Err(format!("生成 SQLite 备份失败：{}", error));
    }

    if let Err(error) = validate_backup(&output).await {
        let _ = std::fs::remove_file(&output);
        return Err(format!("备份完整性校验失败：{}", error));
    }

    Ok(())
}

fn normalize_backup_output(source: &Path, output: &Path) -> Result<PathBuf, String> {
    let source = std::fs::canonicalize(source)
        .map_err(|error| format!("读取源数据库路径失败：{}（{}）", source.display(), error))?;
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let parent = std::fs::canonicalize(parent)
        .map_err(|error| format!("读取备份目标目录失败：{}（{}）", parent.display(), error))?;
    let file_name = output
        .file_name()
        .ok_or_else(|| "备份目标必须包含文件名".to_string())?;
    let output = parent.join(file_name);

    if source == output {
        return Err("备份目标不能覆盖当前数据库".to_string());
    }
    if output.exists() {
        return Err(format!("备份目标已存在：{}", output.display()));
    }

    Ok(output)
}

async fn validate_backup(path: &Path) -> Result<(), String> {
    check_integrity(path.to_path_buf()).await
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
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5));

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
    for migration in SCHEMA_MIGRATIONS {
        let mut transaction = pool
            .begin()
            .await
            .map_err(|error| format!("启动 SQLite 迁移事务失败：{}", error))?;
        let applied: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM schema_migrations WHERE version = ? LIMIT 1")
                .bind(migration.version)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(|error| format!("读取 SQLite 迁移记录失败：{}", error))?;

        if applied.is_some() {
            transaction
                .commit()
                .await
                .map_err(|error| format!("提交 SQLite 迁移事务失败：{}", error))?;
            continue;
        }

        apply_schema_migration(&mut transaction, migration)
            .await
            .map_err(|error| format!("执行 SQLite 迁移 {} 失败：{}", migration.name, error))?;
        sqlx::query("INSERT INTO schema_migrations (version, name, applied_at) VALUES (?, ?, ?)")
            .bind(migration.version)
            .bind(migration.name)
            .bind(current_timestamp_ms())
            .execute(&mut *transaction)
            .await
            .map_err(|error| format!("记录 SQLite 迁移 {} 失败：{}", migration.name, error))?;
        transaction
            .commit()
            .await
            .map_err(|error| format!("提交 SQLite 迁移 {} 失败：{}", migration.name, error))?;
    }

    Ok(())
}

#[derive(Clone, Copy)]
struct SchemaMigration {
    version: i64,
    name: &'static str,
}

// 新迁移只能追加到此列表末尾，已发布的版本号和执行内容不得修改。
const SCHEMA_MIGRATIONS: &[SchemaMigration] = &[
    SchemaMigration {
        version: 1,
        name: "legacy_download_tasks_baseline",
    },
    SchemaMigration {
        version: 2,
        name: "task_operations",
    },
    SchemaMigration {
        version: 3,
        name: "task_query_indexes",
    },
    SchemaMigration {
        version: 4,
        name: "task_proxy_state",
    },
    SchemaMigration {
        version: 5,
        name: "web_auth_jwt_secret",
    },
];

async fn apply_schema_migration(
    transaction: &mut Transaction<'_, Sqlite>,
    migration: &SchemaMigration,
) -> Result<(), String> {
    match migration.version {
        1 => migrate_legacy_download_tasks(transaction).await,
        2 => create_task_operations_schema(transaction).await,
        3 => create_task_query_indexes(transaction).await,
        4 => create_task_proxy_schema(transaction).await,
        5 => create_web_auth_jwt_schema(transaction).await,
        version => Err(format!("未注册 SQLite 迁移版本 {}", version)),
    }
}

async fn create_web_auth_jwt_schema(
    transaction: &mut Transaction<'_, Sqlite>,
) -> Result<(), String> {
    let column_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('web_auth_config') WHERE name = 'jwt_secret'",
    )
    .fetch_one(&mut **transaction)
    .await
    .map_err(|error| format!("检查 Web 鉴权 JWT 密钥字段失败：{}", error))?;
    if column_count == 0 {
        sqlx::query("ALTER TABLE web_auth_config ADD COLUMN jwt_secret TEXT")
            .execute(&mut **transaction)
            .await
            .map_err(|error| format!("迁移 Web 鉴权 JWT 密钥字段失败：{}", error))?;
    }

    let ids: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM web_auth_config WHERE jwt_secret IS NULL OR TRIM(jwt_secret) = ''",
    )
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| format!("读取待生成 JWT 密钥的鉴权记录失败：{}", error))?;
    for id in ids {
        let secret = crate::auth::generate_jwt_secret();
        sqlx::query("UPDATE web_auth_config SET jwt_secret = ? WHERE id = ?")
            .bind(secret)
            .bind(id)
            .execute(&mut **transaction)
            .await
            .map_err(|error| format!("写入 Web 鉴权 JWT 密钥失败：{}", error))?;
    }
    Ok(())
}

async fn create_task_proxy_schema(transaction: &mut Transaction<'_, Sqlite>) -> Result<(), String> {
    if download_tasks_column_count(transaction, "use_proxy").await? == 0 {
        sqlx::query("ALTER TABLE download_tasks ADD COLUMN use_proxy INTEGER NOT NULL DEFAULT 0")
            .execute(&mut **transaction)
            .await
            .map_err(|error| format!("迁移任务代理开关字段失败：{}", error))?;
    }
    if download_tasks_column_count(transaction, "proxy_source").await? == 0 {
        sqlx::query(
            "ALTER TABLE download_tasks ADD COLUMN proxy_source TEXT NOT NULL DEFAULT 'profile'",
        )
        .execute(&mut **transaction)
        .await
        .map_err(|error| format!("迁移任务代理来源字段失败：{}", error))?;
    }
    sqlx::query(TASK_PROXY_OVERRIDES_SCHEMA)
        .execute(&mut **transaction)
        .await
        .map_err(|error| format!("创建任务私密代理覆盖表失败：{}", error))?;
    Ok(())
}

async fn create_task_query_indexes(
    transaction: &mut Transaction<'_, Sqlite>,
) -> Result<(), String> {
    for statement in TASK_QUERY_INDEX_STATEMENTS {
        sqlx::query(statement)
            .execute(&mut **transaction)
            .await
            .map_err(|error| format!("创建任务查询索引失败：{}", error))?;
    }
    Ok(())
}

async fn create_task_operations_schema(
    transaction: &mut Transaction<'_, Sqlite>,
) -> Result<(), String> {
    for statement in TASK_OPERATIONS_SCHEMA_STATEMENTS {
        sqlx::query(statement)
            .execute(&mut **transaction)
            .await
            .map_err(|error| format!("创建任务操作数据表失败：{}", error))?;
    }
    Ok(())
}

async fn migrate_legacy_download_tasks(
    transaction: &mut Transaction<'_, Sqlite>,
) -> Result<(), String> {
    if download_tasks_column_count(transaction, "source_type").await? == 0 {
        sqlx::query(
            "ALTER TABLE download_tasks ADD COLUMN source_type TEXT NOT NULL DEFAULT 'url'",
        )
        .execute(&mut **transaction)
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
        .execute(&mut **transaction)
        .await
        .map_err(|error| format!("回填下载任务来源字段失败：{}", error))?;
    }

    if download_tasks_column_count(transaction, "category").await? == 0 {
        sqlx::query("ALTER TABLE download_tasks ADD COLUMN category TEXT NOT NULL DEFAULT '默认'")
            .execute(&mut **transaction)
            .await
            .map_err(|error| format!("迁移下载任务分类字段失败：{}", error))?;
    }

    if download_tasks_column_count(transaction, "confirmation_required").await? == 0 {
        sqlx::query(
            "ALTER TABLE download_tasks ADD COLUMN confirmation_required INTEGER NOT NULL DEFAULT 0",
        )
        .execute(&mut **transaction)
        .await
        .map_err(|error| format!("迁移下载任务文件确认字段失败：{}", error))?;
    }

    if download_tasks_column_count(transaction, "metadata_torrent_path").await? == 0 {
        sqlx::query("ALTER TABLE download_tasks ADD COLUMN metadata_torrent_path TEXT")
            .execute(&mut **transaction)
            .await
            .map_err(|error| format!("迁移磁链 metadata 路径字段失败：{}", error))?;
    }

    if download_tasks_column_count(transaction, "files_deleted").await? == 0 {
        sqlx::query(
            "ALTER TABLE download_tasks ADD COLUMN files_deleted INTEGER NOT NULL DEFAULT 0",
        )
        .execute(&mut **transaction)
        .await
        .map_err(|error| format!("迁移任务本地文件删除标记失败：{}", error))?;
    }

    if download_tasks_column_count(transaction, "selected_file_indexes").await? == 0 {
        sqlx::query(
            "ALTER TABLE download_tasks ADD COLUMN selected_file_indexes TEXT NOT NULL DEFAULT '[]'",
        )
        .execute(&mut **transaction)
        .await
        .map_err(|error| format!("迁移任务文件选择字段失败：{}", error))?;
    }

    if download_tasks_column_count(transaction, "owned_task_dir").await? == 0 {
        sqlx::query("ALTER TABLE download_tasks ADD COLUMN owned_task_dir TEXT")
            .execute(&mut **transaction)
            .await
            .map_err(|error| format!("迁移任务专属目录字段失败：{}", error))?;
    }

    // 旧版本创建的 UI 偏好表从未承载已上线功能，移除预留接口时同步清理遗留空表。
    sqlx::query("DROP TABLE IF EXISTS ui_preferences")
        .execute(&mut **transaction)
        .await
        .map_err(|error| format!("清理旧 UI 偏好表失败：{}", error))?;

    Ok(())
}

async fn download_tasks_column_count(
    transaction: &mut Transaction<'_, Sqlite>,
    column: &str,
) -> Result<i64, String> {
    sqlx::query_scalar("SELECT COUNT(*) FROM pragma_table_info('download_tasks') WHERE name = ?")
        .bind(column)
        .fetch_one(&mut **transaction)
        .await
        .map_err(|error| format!("检查下载任务字段 {} 失败：{}", column, error))
}

fn current_timestamp_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(i64::MAX)
}

const SCHEMA_STATEMENTS: &[&str] = &[
    r#"
    CREATE TABLE IF NOT EXISTS download_tasks (
        id INTEGER PRIMARY KEY,
        url TEXT NOT NULL,
        source_type TEXT NOT NULL DEFAULT 'url',
        file_name TEXT NOT NULL,
        save_dir TEXT NOT NULL,
        owned_task_dir TEXT,
        category TEXT NOT NULL DEFAULT '默认',
        gid TEXT,
        status TEXT NOT NULL,
        total_length INTEGER NOT NULL DEFAULT 0,
        completed_length INTEGER NOT NULL DEFAULT 0,
        download_speed INTEGER NOT NULL DEFAULT 0,
        error_code TEXT,
        error_message TEXT,
        file_path TEXT,
        use_proxy INTEGER NOT NULL DEFAULT 0,
        proxy_source TEXT NOT NULL DEFAULT 'profile',
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
        auth_version INTEGER NOT NULL CHECK (auth_version > 0),
        jwt_secret TEXT
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS schema_migrations (
        version INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        applied_at INTEGER NOT NULL
    )
    "#,
    TASK_OPERATIONS_SCHEMA_STATEMENTS[0],
    TASK_OPERATIONS_SCHEMA_STATEMENTS[1],
    TASK_PROXY_OVERRIDES_SCHEMA,
];

const TASK_PROXY_OVERRIDES_SCHEMA: &str = r#"
    CREATE TABLE IF NOT EXISTS task_proxy_overrides (
        task_id INTEGER PRIMARY KEY,
        proxy_url TEXT NOT NULL,
        updated_at INTEGER NOT NULL
    )
    "#;

const TASK_OPERATIONS_SCHEMA_STATEMENTS: &[&str] = &[
    r#"
    CREATE TABLE IF NOT EXISTS task_operations (
        id TEXT PRIMARY KEY,
        task_id INTEGER NOT NULL,
        operation_type TEXT NOT NULL,
        phase TEXT NOT NULL,
        context_json TEXT NOT NULL,
        error_message TEXT,
        status TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    )
    "#,
    r#"
    CREATE INDEX IF NOT EXISTS idx_task_operations_unfinished
    ON task_operations (status, updated_at)
    "#,
];

const TASK_QUERY_INDEX_STATEMENTS: &[&str] = &[
    r#"
    CREATE INDEX IF NOT EXISTS idx_download_tasks_status_updated_at
    ON download_tasks (status, updated_at)
    "#,
    r#"
    CREATE INDEX IF NOT EXISTS idx_task_history_task_created_at
    ON task_history (task_id, created_at DESC, id DESC)
    "#,
    r#"
    CREATE INDEX IF NOT EXISTS idx_task_errors_task_created_at
    ON task_errors (task_id, created_at DESC, id DESC)
    "#,
    r#"
    CREATE INDEX IF NOT EXISTS idx_task_operations_unfinished_created_at
    ON task_operations (status, created_at, id)
    "#,
];

#[cfg(test)]
mod tests;
