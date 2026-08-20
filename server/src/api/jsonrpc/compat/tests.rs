use super::model::{Aria2CompatFile, Aria2CompatTask, Aria2GlobalStat};
use super::params::{parse_method, CompatCommand, ControlOperation, TaskLane, MAX_PAGE_SIZE};
use crate::tasks::{
    DownloadTask, DownloadTaskFile, DownloadTaskSourceType, DownloadTaskStatus, TaskProxyBinding,
};
use serde_json::json;

#[test]
fn parses_all_extension_method_shapes_without_sidecar() {
    let cases = [
        ("aria2.getGlobalStat", json!(["token:test"])),
        ("aria2.tellActive", json!(["token:test", ["gid", "status"]])),
        (
            "aria2.tellWaiting",
            json!(["token:test", -2, 20, ["gid", "files"]]),
        ),
        ("aria2.tellStopped", json!(["token:test", 0, 20, ["gid"]])),
        ("aria2.pause", json!(["token:test", "gid-1"])),
        ("aria2.unpause", json!(["token:test", "gid-1"])),
        ("aria2.remove", json!(["token:test", "gid-1"])),
        ("aria2.removeDownloadResult", json!(["token:test", "gid-1"])),
        ("aria2.pauseAll", json!(["token:test"])),
        ("aria2.unpauseAll", json!(["token:test"])),
        ("aria2.purgeDownloadResult", json!(["token:test"])),
    ];

    assert!(matches!(
        parse_method(cases[0].0, &cases[0].1),
        Ok(CompatCommand::GlobalStat)
    ));
    assert!(matches!(
        parse_method(cases[1].0, &cases[1].1),
        Ok(CompatCommand::Tell {
            lane: TaskLane::Active,
            num: MAX_PAGE_SIZE,
            ..
        })
    ));
    assert!(matches!(
        parse_method(cases[2].0, &cases[2].1),
        Ok(CompatCommand::Tell {
            lane: TaskLane::Waiting,
            offset: -2,
            num: 20,
            ..
        })
    ));
    assert!(matches!(
        parse_method(cases[3].0, &cases[3].1),
        Ok(CompatCommand::Tell {
            lane: TaskLane::Stopped,
            ..
        })
    ));

    for (method, params) in &cases[4..8] {
        assert!(matches!(
            parse_method(method, params),
            Ok(CompatCommand::Control {
                operation: ControlOperation::Pause
                    | ControlOperation::Unpause
                    | ControlOperation::Remove
                    | ControlOperation::RemoveDownloadResult,
                gid: Some(_),
            })
        ));
    }
    for (method, params) in &cases[8..] {
        assert!(matches!(
            parse_method(method, params),
            Ok(CompatCommand::Control { gid: None, .. })
        ));
    }
}

#[test]
fn rejects_unknown_keys_and_large_pages() {
    let unknown = parse_method(
        "aria2.tellActive",
        &json!(["token:test", ["gid", "unknown"]]),
    )
    .expect_err("unknown keys must be rejected");
    assert_eq!(unknown.code, -32602);

    let too_large = parse_method(
        "aria2.tellWaiting",
        &json!(["token:test", 0, MAX_PAGE_SIZE + 1]),
    )
    .expect_err("large pages must be rejected");
    assert_eq!(too_large.code, -32602);
}

#[test]
fn serializes_compat_task_fields_with_aria2_string_types() {
    let task = Aria2CompatTask {
        gid: "gid-1".to_string(),
        status: "active".to_string(),
        total_length: 100,
        completed_length: 25,
        download_speed: 4,
        error_code: "0".to_string(),
        error_message: None,
        dir: Some("/downloads".to_string()),
        files: vec![Aria2CompatFile {
            index: 1,
            path: "/downloads/file.zip".to_string(),
            length: 100,
            completed_length: 25,
            selected: true,
        }],
        bittorrent_name: None,
    };
    let value = task.to_value(&[
        "gid".to_string(),
        "status".to_string(),
        "totalLength".to_string(),
        "files".to_string(),
    ]);
    assert_eq!(value["gid"], json!("gid-1"));
    assert_eq!(value["totalLength"], json!("100"));
    assert_eq!(value["files"][0]["index"], json!("1"));
    assert_eq!(value["files"][0]["selected"], json!("true"));
}

#[test]
fn serializes_global_stat_with_all_required_string_fields() {
    let value = Aria2GlobalStat {
        download_speed: 12,
        num_active: 1,
        num_waiting: 2,
        num_stopped: 3,
    }
    .to_value();
    for key in [
        "downloadSpeed",
        "uploadSpeed",
        "numActive",
        "numWaiting",
        "numStopped",
        "numStoppedTotal",
    ] {
        assert!(value[key].is_string(), "{key} must be a string");
    }
    assert_eq!(value["numStoppedTotal"], json!("3"));
}

#[test]
fn converts_download_tasks_with_status_defaults_and_safe_paths() {
    let cases = [
        (DownloadTaskStatus::Pending, "waiting"),
        (DownloadTaskStatus::Active, "active"),
        (DownloadTaskStatus::Paused, "paused"),
        (DownloadTaskStatus::Complete, "complete"),
        (DownloadTaskStatus::Error, "error"),
    ];

    for (status, expected) in cases {
        let mut task = download_task(status);
        task.error_code = Some("not-a-number".to_string());
        task.error_message =
            Some("download failed: https://example.com/file.zip?token=secret".to_string());
        task.files.push(DownloadTaskFile {
            index: 2,
            path: "/downloads/../private/secret.txt".to_string(),
            name: "secret.txt".to_string(),
            length: 1,
            completed_length: 0,
            selected: true,
        });

        let compat = Aria2CompatTask::from_download_task_with_paths(&task, &[])
            .expect("visible task with a GID should convert");
        assert_eq!(compat.status, expected);
        assert_eq!(compat.error_code, "0");
        assert_eq!(compat.files.len(), 1);
        if task.status == DownloadTaskStatus::Error {
            assert_eq!(
                compat.error_message.as_deref(),
                Some("download failed: https://example.com/file.zip")
            );
        } else {
            assert_eq!(compat.error_message, None);
        }
    }
}

#[test]
fn hides_removed_and_gidless_tasks() {
    let removed = download_task(DownloadTaskStatus::Removed);
    assert!(Aria2CompatTask::from_download_task_with_paths(&removed, &[]).is_none());

    let mut gidless = download_task(DownloadTaskStatus::Active);
    gidless.gid = None;
    assert!(Aria2CompatTask::from_download_task_with_paths(&gidless, &[]).is_none());
}

#[test]
fn exposes_bt_name_and_magnet_metadata_with_their_current_gid() {
    let mut torrent = download_task(DownloadTaskStatus::Active);
    torrent.source_type = DownloadTaskSourceType::Torrent;
    torrent.file_name = "Ubuntu.iso".to_string();
    let torrent_compat = Aria2CompatTask::from_download_task_with_paths(&torrent, &[])
        .expect("torrent task with a GID should convert");
    assert_eq!(
        torrent_compat.bittorrent_name.as_deref(),
        Some("Ubuntu.iso")
    );

    let mut magnet = download_task(DownloadTaskStatus::Pending);
    magnet.source_type = DownloadTaskSourceType::Magnet;
    magnet.file_name = "metadata-name".to_string();
    magnet.confirmation_required = true;
    let magnet_compat = Aria2CompatTask::from_download_task_with_paths(&magnet, &[])
        .expect("magnet metadata task with a temporary GID should convert");
    assert_eq!(magnet_compat.status, "waiting");
    assert_eq!(
        magnet_compat.bittorrent_name.as_deref(),
        Some("metadata-name")
    );
}

#[cfg(unix)]
#[test]
fn strict_path_conversion_rejects_symlink_escape() {
    use std::os::unix::fs::symlink;

    let suffix = format!("{}-{}", std::process::id(), 1);
    let root = std::env::temp_dir().join(format!("motrix-compat-root-{suffix}"));
    let outside = std::env::temp_dir().join(format!("motrix-compat-outside-{suffix}"));
    std::fs::create_dir_all(&root).expect("root should create");
    std::fs::create_dir_all(&outside).expect("outside should create");
    let outside_file = outside.join("secret.bin");
    std::fs::write(&outside_file, b"secret").expect("outside file should write");
    let link = root.join("link.bin");
    symlink(&outside_file, &link).expect("symlink should create");

    let mut task = download_task(DownloadTaskStatus::Complete);
    task.save_dir = root.display().to_string();
    task.file_path = Some(link.display().to_string());
    task.files.clear();
    let compat =
        Aria2CompatTask::from_download_task_with_paths(&task, &[root.display().to_string()])
            .expect("task should retain safe fields");

    assert_eq!(compat.dir.as_deref(), Some(root.to_str().unwrap()));
    assert!(compat.files.is_empty());
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&outside);
}

fn download_task(status: DownloadTaskStatus) -> DownloadTask {
    DownloadTask {
        id: 1,
        url: "https://example.com/file.zip".to_string(),
        source_type: DownloadTaskSourceType::Url,
        file_name: "file.zip".to_string(),
        save_dir: "/downloads".to_string(),
        owned_task_dir: None,
        category: "default".to_string(),
        gid: Some("gid-1".to_string()),
        status,
        total_length: 100,
        completed_length: 25,
        download_speed: 4,
        error_code: None,
        error_message: None,
        file_path: None,
        use_proxy: false,
        proxy_binding: TaskProxyBinding::default(),
        metadata_torrent_path: None,
        files_deleted: false,
        selected_file_indexes: vec![1],
        confirmation_required: false,
        files: vec![DownloadTaskFile {
            index: 1,
            path: "/downloads/file.zip".to_string(),
            name: "file.zip".to_string(),
            length: 100,
            completed_length: 25,
            selected: true,
        }],
        created_at: 1,
        updated_at: 1,
    }
}
