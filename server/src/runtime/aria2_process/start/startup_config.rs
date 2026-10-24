use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

pub(super) struct StartupConfig(PathBuf);

impl StartupConfig {
    pub(super) fn new(app_data_dir: &Path, proxy: &str) -> Result<Self, String> {
        if proxy.chars().any(char::is_control) {
            return Err("BT 启动代理配置包含控制字符".to_string());
        }
        let directory = crate::state::ensure_private_directory(
            app_data_dir,
            Path::new("engine/native-v2/startup"),
        )?;
        let path = directory.join(format!("{}.conf", crate::aria2::generate_rpc_secret()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(|_| "无法创建 BT 私有启动配置".to_string())?;
        let config = Self(path);
        // 凭据只经私有文件传给引擎，不能出现在进程参数或启动摘要中。
        writeln!(file, "bt-proxy={proxy}")
            .and_then(|_| file.sync_all())
            .map_err(|_| "无法写入 BT 私有启动配置".to_string())?;
        Ok(config)
    }

    pub(super) fn apply_args(&self, args: &mut Vec<String>) {
        args.retain(|arg| arg != "--no-conf=true");
        args.push(format!("--conf-path={}", self.0.display()));
    }
}

impl Drop for StartupConfig {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests;
