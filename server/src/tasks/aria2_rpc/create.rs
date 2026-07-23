use super::transport::rpc_params;
use crate::aria2::{Aria2RpcClient, Aria2RpcError};
use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use crate::tasks::{
    log_error, log_info, redact_url_for_log, DownloadTaskSourceType, DownloadTaskStartMode,
    PreparedDownloadTask,
};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;

const DEFAULT_BT_TRACKERS: &[&str] = &[
    "udp://tracker.opentrackr.org:1337/announce",
    "udp://open.stealth.si:80/announce",
    "udp://tracker.openbittorrent.com:80/announce",
    "udp://exodus.desync.com:6969/announce",
    "udp://tracker.torrent.eu.org:451/announce",
    "udp://open.demonii.com:1337/announce",
];

pub async fn add_uri_to_aria2(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    task: &PreparedDownloadTask,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    log_info(
        debug_logs,
        "aria2.addUri",
        format!(
            "开始创建 Aria2 下载任务，URL {}，保存目录 {}",
            redact_url_for_log(&task.url),
            task.aria2_save_dir.as_deref().unwrap_or(&task.save_dir)
        ),
    );
    let request_body = super::build_add_uri_request(config, task);
    let gid = match client
        .request::<String>(config, &request_body)
        .await
        .and_then(|response| response.into_result())
    {
        Ok(gid) if !gid.trim().is_empty() => gid,
        Ok(_) => {
            let error = "创建 Aria2 下载任务失败：响应缺少 GID".to_string();
            log_error(debug_logs, "aria2.addUri", &error);
            return Err(error);
        }
        Err(error) => {
            let error = if matches!(&error, Aria2RpcError::ConnectionFailed(_)) {
                "创建下载任务失败：无法连接 Aria2 RPC，请确认引擎已启动".to_string()
            } else {
                format!("创建下载任务失败：{}", error)
            };
            log_error(debug_logs, "aria2.addUri", &error);
            return Err(error);
        }
    };
    log_info(
        debug_logs,
        "aria2.addUri",
        format!("Aria2 下载任务创建成功，GID {}", gid),
    );
    Ok(gid)
}

pub async fn add_torrent_to_aria2(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    task: &PreparedDownloadTask,
    torrent_data: &[u8],
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    log_info(
        debug_logs,
        "aria2.addTorrent",
        format!(
            "开始创建 Aria2 种子任务，文件 {}，保存目录 {}",
            task.file_name, task.save_dir
        ),
    );
    let request_body = super::build_add_torrent_request(config, task, torrent_data);
    let gid = match client
        .request::<String>(config, &request_body)
        .await
        .and_then(|response| response.into_result())
    {
        Ok(gid) if !gid.trim().is_empty() => gid,
        Ok(_) => {
            let error = "创建种子任务失败：响应缺少 GID".to_string();
            log_error(debug_logs, "aria2.addTorrent", &error);
            return Err(error);
        }
        Err(error) => {
            let error = if matches!(&error, Aria2RpcError::ConnectionFailed(_)) {
                "创建种子任务失败：无法连接 Aria2 RPC，请确认引擎已启动".to_string()
            } else {
                format!("创建种子任务失败：{}", error)
            };
            log_error(debug_logs, "aria2.addTorrent", &error);
            return Err(error);
        }
    };
    log_info(
        debug_logs,
        "aria2.addTorrent",
        format!("Aria2 种子任务创建成功，GID {}", gid),
    );
    Ok(gid)
}

pub(crate) fn build_add_uri_request(
    config: &Aria2Config,
    task: &PreparedDownloadTask,
) -> serde_json::Value {
    let mut params = rpc_params(config);

    params.push(serde_json::json!([task.url.clone()]));

    let mut options = serde_json::Map::new();
    for (key, value) in task.aria2_options.clone() {
        options.insert(key, value);
    }
    if task.source_type == DownloadTaskSourceType::Magnet {
        apply_start_mode_option(&mut options, DownloadTaskStartMode::Now);
        options.insert("pause-metadata".to_string(), serde_json::json!("true"));
        options.insert("bt-save-metadata".to_string(), serde_json::json!("true"));
        apply_default_bt_trackers(&mut options);
    } else {
        apply_start_mode_option(&mut options, task.start_mode);
    }
    options.insert(
        "dir".to_string(),
        serde_json::json!(task.aria2_save_dir.as_deref().unwrap_or(&task.save_dir)),
    );
    if task.source_type == DownloadTaskSourceType::Url {
        if let Some(output_file_name) = task.output_file_name.as_deref() {
            options.insert("out".to_string(), serde_json::json!(output_file_name));
        }
    }
    params.push(serde_json::Value::Object(options));

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": "motrix-fnos-add-uri",
        "method": "aria2.addUri",
        "params": params,
    })
}

fn apply_start_mode_option(
    options: &mut serde_json::Map<String, serde_json::Value>,
    start_mode: DownloadTaskStartMode,
) {
    let pause = match start_mode {
        DownloadTaskStartMode::Now => "false",
        DownloadTaskStartMode::Paused => "true",
    };
    options.insert("pause".to_string(), serde_json::json!(pause));
}

fn apply_default_bt_trackers(options: &mut serde_json::Map<String, serde_json::Value>) {
    options
        .entry("bt-tracker".to_string())
        .or_insert_with(|| serde_json::json!(DEFAULT_BT_TRACKERS.join(",")));
}

fn apply_default_bt_seed_behavior(options: &mut serde_json::Map<String, serde_json::Value>) {
    options
        .entry("seed-time".to_string())
        .or_insert_with(|| serde_json::json!("0"));
}

pub(crate) fn build_add_torrent_request(
    config: &Aria2Config,
    task: &PreparedDownloadTask,
    torrent_data: &[u8],
) -> serde_json::Value {
    let mut params = rpc_params(config);

    params.push(serde_json::json!(STANDARD.encode(torrent_data)));
    params.push(serde_json::json!([]));

    let mut options = serde_json::Map::new();
    for (key, value) in task.aria2_options.clone() {
        options.insert(key, value);
    }
    apply_start_mode_option(&mut options, task.start_mode);
    apply_default_bt_trackers(&mut options);
    apply_default_bt_seed_behavior(&mut options);
    if task.start_mode == DownloadTaskStartMode::Paused {
        options.insert("pause-metadata".to_string(), serde_json::json!("true"));
    }
    options.insert(
        "dir".to_string(),
        serde_json::json!(task.aria2_save_dir.as_deref().unwrap_or(&task.save_dir)),
    );
    params.push(serde_json::Value::Object(options));

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": "motrix-fnos-add-torrent",
        "method": "aria2.addTorrent",
        "params": params,
    })
}
