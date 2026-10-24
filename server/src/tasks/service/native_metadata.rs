use super::*;
use base64::Engine;
use bencode::{encode_bytes, invalid, parse};
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;

mod bencode;
mod layers;

impl TaskService<'_> {
    pub(super) async fn recreation_torrent_metadata(
        &self,
        config: &Aria2Config,
        task: &DownloadTask,
    ) -> Result<Vec<u8>, String> {
        if task.engine_generation != crate::config::aria2::EngineGeneration::NativeV2
            || task.source_type != DownloadTaskSourceType::Magnet
        {
            return read_saved_torrent_metadata(task);
        }
        let hashes = magnet_hashes(&task.url)?;
        let backup_dir = crate::state::ensure_private_directory(
            self.app_data_dir,
            Path::new(&format!("task-metadata/task-{}", task.id)),
        )?;
        let backup = backup_dir.join("source.torrent");
        if task
            .metadata_torrent_path
            .as_deref()
            .is_some_and(|path| Path::new(path) != backup)
        {
            return Err(invalid());
        }
        let (data, save_backup) = match read_private_file(&backup) {
            Ok(data) => (data, false),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if task.metadata_torrent_path.is_some() {
                    return Err(invalid());
                }
                crate::aria2::save_session(self.aria2_rpc, config, Some(self.debug_logs)).await?;
                let directory = crate::state::ensure_private_directory(
                    self.app_data_dir,
                    Path::new("engine/native-v2/state/bittorrent/torrents"),
                )?;
                let state_root = directory
                    .parent()
                    .and_then(Path::parent)
                    .ok_or_else(invalid)?;
                if config
                    .state_dir
                    .as_deref()
                    .and_then(|path| fs::canonicalize(path).ok())
                    .as_deref()
                    != Some(state_root)
                {
                    return Err(invalid());
                }
                let hash = hashes
                    .0
                    .as_ref()
                    .or(hashes.1.as_ref())
                    .ok_or_else(invalid)?;
                let resume = read_private_file(&directory.join(format!("{hash}.fastresume")))
                    .map_err(|_| invalid())?;
                (torrent_from_resume(&resume)?, true)
            }
            Err(_) => return Err(invalid()),
        };
        if data.len() > 10 * 1024 * 1024 {
            return Err(invalid());
        }
        let inspected = self.aria2_rpc.request::<serde_json::Value>(config, &serde_json::json!({
            "jsonrpc": "2.0", "id": format!("motrix-fnos-metadata-{}", task.id),
            "method": "aria2.inspectTorrent",
            "params": [format!("token:{}", config.rpc_secret), base64::engine::general_purpose::STANDARD.encode(&data)],
        })).await.map_err(|_| invalid())?.into_result().map_err(|_| invalid())?;
        validate_inspection(task, &hashes, &inspected)?;
        if save_backup {
            // 备份只服务显式从头下载，原生续传仍依赖原 GID 和引擎状态。
            save_restore_torrent_metadata(self.app_data_dir, task.id, &data)?;
        }
        Ok(data)
    }
}

fn read_private_file(path: &Path) -> std::io::Result<Vec<u8>> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > 64 * 1024 * 1024 {
        return Err(std::io::Error::other("invalid metadata file"));
    }
    let mut data = Vec::new();
    file.take(64 * 1024 * 1024 + 1).read_to_end(&mut data)?;
    if data.len() > 64 * 1024 * 1024 {
        return Err(std::io::Error::other("metadata too large"));
    }
    Ok(data)
}

fn torrent_from_resume(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let resume = parse(bytes)?;
    let info = resume.get(b"info")?;
    info.dict()?;
    let mut entries = std::collections::BTreeMap::new();
    entries.insert(b"info".as_slice(), info.raw.to_vec());
    for (source, target) in [
        (b"trackers".as_slice(), b"announce-list".as_slice()),
        (b"url-list", b"url-list"),
        (b"httpseeds", b"httpseeds"),
    ] {
        if let Some(node) = resume.dict()?.get(source) {
            node.list()?;
            entries.insert(target, node.raw.to_vec());
        }
    }
    if info.dict()?.contains_key(b"meta version".as_slice()) {
        if info.get(b"meta version")?.integer()? != 2 {
            return Err(invalid());
        }
        let mut encoded = vec![b'd'];
        for (root, layer) in layers::piece_layers(&resume, info)? {
            encode_bytes(&mut encoded, &root);
            encode_bytes(&mut encoded, &layer);
        }
        encoded.push(b'e');
        entries.insert(b"piece layers", encoded);
    }
    let mut torrent = vec![b'd'];
    for (key, value) in entries {
        encode_bytes(&mut torrent, key);
        torrent.extend_from_slice(&value);
    }
    torrent.push(b'e');
    Ok(torrent)
}

fn magnet_hashes(magnet: &str) -> Result<(Option<String>, Option<String>), String> {
    let url = reqwest::Url::parse(magnet).map_err(|_| invalid())?;
    let mut hashes = (None, None);
    for (_, value) in url.query_pairs().filter(|(key, _)| key == "xt") {
        let value = value.to_ascii_lowercase();
        if let Some(hash) = value.strip_prefix("urn:btih:") {
            let hash = if hash.len() == 32 {
                let mut bytes = Vec::new();
                let mut bits = 0u32;
                let mut count = 0;
                for byte in hash.bytes() {
                    let digit = match byte {
                        b'a'..=b'z' => byte - b'a',
                        b'2'..=b'7' => byte - b'2' + 26,
                        _ => return Err(invalid()),
                    };
                    bits = (bits << 5) | u32::from(digit);
                    count += 5;
                    if count >= 8 {
                        count -= 8;
                        bytes.push((bits >> count) as u8);
                    }
                }
                bytes.iter().map(|byte| format!("{byte:02x}")).collect()
            } else {
                hash.to_string()
            };
            if hash.len() != 40 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(invalid());
            }
            if hashes
                .0
                .replace(hash.clone())
                .is_some_and(|old| old != hash)
            {
                return Err(invalid());
            }
        } else if let Some(hash) = value.strip_prefix("urn:btmh:1220") {
            if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(invalid());
            }
            if hashes
                .1
                .replace(hash.to_string())
                .is_some_and(|old| old != hash)
            {
                return Err(invalid());
            }
        }
    }
    if hashes.0.is_none() && hashes.1.is_none() {
        return Err(invalid());
    }
    Ok(hashes)
}

fn validate_inspection(
    task: &DownloadTask,
    hashes: &(Option<String>, Option<String>),
    inspected: &serde_json::Value,
) -> Result<(), String> {
    for (expected, field) in [(&hashes.0, "infoHashV1"), (&hashes.1, "infoHashV2")] {
        if expected
            .as_ref()
            .is_some_and(|hash| inspected[field].as_str() != Some(hash.as_str()))
        {
            return Err(invalid());
        }
    }
    let files = inspected["files"].as_array().ok_or_else(invalid)?;
    if files.is_empty() || files.len() != task.files.len() {
        return Err(invalid());
    }
    for (actual, expected) in files.iter().zip(&task.files) {
        let path = Path::new(&expected.path)
            .strip_prefix(task_download_dir(task))
            .map_err(|_| invalid())?;
        let length = actual["length"]
            .as_str()
            .and_then(|length| length.parse::<u64>().ok());
        let index = actual["index"]
            .as_str()
            .and_then(|index| index.parse::<u32>().ok())
            .or_else(|| {
                actual["index"]
                    .as_u64()
                    .and_then(|index| u32::try_from(index).ok())
            });
        if path.to_str() != actual["path"].as_str()
            || length != Some(expected.length)
            || index != Some(expected.index)
        {
            return Err(invalid());
        }
    }
    if task
        .selected_file_indexes
        .iter()
        .any(|index| !task.files.iter().any(|file| file.index == *index))
    {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
