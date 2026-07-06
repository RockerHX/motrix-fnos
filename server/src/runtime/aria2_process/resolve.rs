use super::types::ResolvedAria2Binary;
use crate::app::ServerRuntimeConfig;
use crate::config::aria2::{Aria2BinarySource, Aria2Config};
use std::path::{Path, PathBuf};

pub fn resolve_aria2_binary(
    runtime: &ServerRuntimeConfig,
    config: &Aria2Config,
) -> Result<ResolvedAria2Binary, String> {
    resolve_aria2_binary_with(
        runtime,
        config,
        std::env::current_exe().ok().as_deref(),
        repo_root_from_manifest_dir().as_deref(),
    )
}

pub(super) fn resolve_aria2_binary_with(
    runtime: &ServerRuntimeConfig,
    config: &Aria2Config,
    current_exe: Option<&Path>,
    repo_root: Option<&Path>,
) -> Result<ResolvedAria2Binary, String> {
    if let Some(path) = runtime.aria2_path.as_deref() {
        return resolve_explicit_binary_path(path);
    }

    if let Some(path) = current_exe
        .and_then(|path| packaged_binary_path(path, &config.sidecar_name))
        .filter(|path| path.is_file())
    {
        return Ok(ResolvedAria2Binary {
            path,
            source: Aria2BinarySource::Sidecar,
        });
    }

    if let Some(path) = repo_root
        .map(|root| repo_debug_binary_path(root, config))
        .filter(|path| path.is_file())
    {
        return Ok(ResolvedAria2Binary {
            path,
            source: Aria2BinarySource::Sidecar,
        });
    }

    Err(format!(
        "未找到可用 Aria2 Next 可执行文件：已检查 MOTRIX_FNOS_ARIA2_PATH、打包目录 bin/{}、仓库调试路径 {}",
        platform_binary_name(&config.sidecar_name),
        repo_root
            .map(|root| repo_debug_binary_path(root, config).display().to_string())
            .unwrap_or_else(|| "<unknown>".to_string())
    ))
}

fn resolve_explicit_binary_path(path: &Path) -> Result<ResolvedAria2Binary, String> {
    if !path.is_file() {
        return Err(format!(
            "MOTRIX_FNOS_ARIA2_PATH 指向的 Aria2 Next 路径不存在或不是文件：{}",
            path.display()
        ));
    }

    Ok(ResolvedAria2Binary {
        path: path.to_path_buf(),
        source: Aria2BinarySource::ExternalPath,
    })
}

fn repo_root_from_manifest_dir() -> Option<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
}

fn packaged_binary_path(current_exe: &Path, sidecar_name: &str) -> Option<PathBuf> {
    current_exe
        .parent()
        .map(|dir| dir.join("bin").join(platform_binary_name(sidecar_name)))
}

pub(super) fn repo_debug_binary_path(repo_root: &Path, config: &Aria2Config) -> PathBuf {
    repo_root.join("assets").join("aria2").join(format!(
        "{}-{}{}",
        config.sidecar_name,
        config.target_triple,
        executable_suffix_for_target(&config.target_triple)
    ))
}

pub(super) fn platform_binary_name(sidecar_name: &str) -> String {
    format!(
        "{sidecar_name}{}",
        executable_suffix_for_target(std::env::consts::OS)
    )
}

fn executable_suffix_for_target(target: &str) -> &'static str {
    if target.contains("windows") {
        ".exe"
    } else {
        ""
    }
}
