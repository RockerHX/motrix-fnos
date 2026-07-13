use crate::api::error::ApiError;
use crate::app::HttpAppState;
use crate::debug_logs::{emit_file_log, DebugLogLevel};
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use reqwest::header::{ACCEPT, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const APP_NAME: &str = "Motrix";
pub const APP_MAINTAINER: &str = "rockerhx";
pub const REPOSITORY_URL: &str = "https://github.com/RockerHX/motrix-fnos";
pub const RELEASE_PAGE_URL: &str = "https://github.com/RockerHX/motrix-fnos/releases";
pub const UPDATE_MODE: &str = "manual_fpk_or_app_center";
const GITHUB_LATEST_RELEASE_API_URL: &str =
    "https://api.github.com/repos/RockerHX/motrix-fnos/releases/latest";
const UPDATE_CHECK_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub backend_status: String,
    pub maintainer: String,
    pub repository_url: String,
    pub release_page_url: String,
    pub target_arch: String,
    pub update_mode: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackendPing {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateCheck {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub has_update: bool,
    pub status: UpdateCheckStatus,
    pub release_url: Option<String>,
    pub assets: Vec<ReleaseAssetInfo>,
    pub checked_at: u64,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateCheckStatus {
    Available,
    UpToDate,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseAssetInfo {
    pub architecture: String,
    pub name: String,
    pub download_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    assets: Vec<GitHubReleaseAsset>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitHubReleaseAsset {
    name: String,
    browser_download_url: String,
}

pub fn routes() -> Router<Arc<HttpAppState>> {
    Router::new()
        .route("/app/info", get(get_app_info))
        .route("/app/ping", get(ping_backend))
        .route("/app/update-check", get(check_update))
}

async fn get_app_info(State(_state): State<Arc<HttpAppState>>) -> Result<Json<AppInfo>, ApiError> {
    emit_file_log(DebugLogLevel::Info, "app", "读取应用信息");
    Ok(Json(AppInfo {
        name: APP_NAME.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        backend_status: "ready".to_string(),
        maintainer: APP_MAINTAINER.to_string(),
        repository_url: REPOSITORY_URL.to_string(),
        release_page_url: RELEASE_PAGE_URL.to_string(),
        target_arch: std::env::consts::ARCH.to_string(),
        update_mode: UPDATE_MODE.to_string(),
    }))
}

async fn ping_backend(
    State(_state): State<Arc<HttpAppState>>,
) -> Result<Json<BackendPing>, ApiError> {
    emit_file_log(DebugLogLevel::Info, "app", "Rust 后端通信检查成功");
    Ok(Json(BackendPing {
        ok: true,
        message: "Rust 后端通信正常".to_string(),
    }))
}

async fn check_update(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<AppUpdateCheck>, ApiError> {
    state.core.debug_logs.info("app.update", "开始检查应用更新");
    let current_version = env!("CARGO_PKG_VERSION");
    let response = match fetch_latest_release().await {
        Ok(release) => update_check_from_release(current_version, release),
        Err(error) => unavailable_update_check(current_version, error),
    };
    Ok(Json(response))
}

async fn fetch_latest_release() -> Result<GitHubRelease, String> {
    let client = reqwest::Client::builder()
        .timeout(UPDATE_CHECK_TIMEOUT)
        .build()
        .map_err(|error| format!("创建版本检测客户端失败：{}", error))?;
    client
        .get(GITHUB_LATEST_RELEASE_API_URL)
        .header(
            USER_AGENT,
            format!("motrix-fnos/{}", env!("CARGO_PKG_VERSION")),
        )
        .header(ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| format!("请求 GitHub Release 失败：{}", error))?
        .error_for_status()
        .map_err(|error| format!("GitHub Release 响应异常：{}", error))?
        .json::<GitHubRelease>()
        .await
        .map_err(|error| format!("解析 GitHub Release 失败：{}", error))
}

fn update_check_from_release(current_version: &str, release: GitHubRelease) -> AppUpdateCheck {
    let latest_version = normalize_release_version(&release.tag_name);
    let assets = release_assets(release.assets);
    let has_update = compare_versions(&latest_version, current_version) == Ordering::Greater;
    let status = if has_update {
        UpdateCheckStatus::Available
    } else {
        UpdateCheckStatus::UpToDate
    };
    let message = if has_update {
        "检测到新版本，请下载匹配设备架构的 FPK 后在 fnOS 应用中心手动安装。"
    } else {
        "当前已是最新版本。"
    };

    AppUpdateCheck {
        current_version: current_version.to_string(),
        latest_version: Some(latest_version),
        has_update,
        status,
        release_url: Some(release.html_url),
        assets,
        checked_at: current_timestamp_ms(),
        message: message.to_string(),
    }
}

fn unavailable_update_check(current_version: &str, message: impl Into<String>) -> AppUpdateCheck {
    AppUpdateCheck {
        current_version: current_version.to_string(),
        latest_version: None,
        has_update: false,
        status: UpdateCheckStatus::Unavailable,
        release_url: Some(RELEASE_PAGE_URL.to_string()),
        assets: Vec::new(),
        checked_at: current_timestamp_ms(),
        message: format!(
            "版本检测失败：{}。可手动前往 Release 页面查看最新版本。",
            message.into()
        ),
    }
}

fn release_assets(assets: Vec<GitHubReleaseAsset>) -> Vec<ReleaseAssetInfo> {
    assets
        .into_iter()
        .filter_map(|asset| {
            let architecture = if asset.name.ends_with("_x86.fpk") {
                "x86"
            } else if asset.name.ends_with("_arm.fpk") {
                "arm"
            } else {
                return None;
            };
            Some(ReleaseAssetInfo {
                architecture: architecture.to_string(),
                name: asset.name,
                download_url: asset.browser_download_url,
            })
        })
        .collect()
}

fn normalize_release_version(version: &str) -> String {
    version.trim().trim_start_matches(['v', 'V']).to_string()
}

fn compare_versions(left: &str, right: &str) -> Ordering {
    let left = version_segments(left);
    let right = version_segments(right);
    let len = left.len().max(right.len());
    for index in 0..len {
        let left_part = left.get(index).copied().unwrap_or_default();
        let right_part = right.get(index).copied().unwrap_or_default();
        match left_part.cmp(&right_part) {
            Ordering::Equal => continue,
            other => return other,
        }
    }
    Ordering::Equal
}

fn version_segments(version: &str) -> Vec<u64> {
    normalize_release_version(version)
        .split(['.', '-'])
        .map(|part| part.parse::<u64>().unwrap_or_default())
        .collect()
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
