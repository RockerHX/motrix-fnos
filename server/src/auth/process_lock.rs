use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::path::Path;

pub struct ServerProcessLock {
    _file: File,
}

impl ServerProcessLock {
    pub fn acquire(app_data_dir: &Path) -> Result<Self, String> {
        let runtime_dir = app_data_dir.join("run");
        std::fs::create_dir_all(&runtime_dir).map_err(|error| {
            format!(
                "创建服务运行目录失败：{}（{}）",
                runtime_dir.display(),
                error
            )
        })?;
        let lock_path = runtime_dir.join("motrix-fnos-server.lock");
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .map_err(|error| format!("打开服务进程锁失败：{}（{}）", lock_path.display(), error))?;
        file.try_lock_exclusive().map_err(|error| {
            format!(
                "motrix-fnos-server 正在运行，请先停止应用后重试（{}）",
                error
            )
        })?;
        Ok(Self { _file: file })
    }
}
