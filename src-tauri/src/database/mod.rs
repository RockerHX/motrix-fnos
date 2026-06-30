use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::str::FromStr;
use tauri::Manager;

pub const DATABASE_FILE_NAME: &str = "motrix-fnos.sqlite";

#[derive(Debug, Clone)]
pub struct AppDatabase {
    pub pool: SqlitePool,
    pub path: PathBuf,
}

pub fn database_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("读取应用数据目录失败：{}", error))?;

    Ok(app_data_dir.join(DATABASE_FILE_NAME))
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

    Ok(AppDatabase { pool, path })
}

