use super::*;
use sha2::{Digest, Sha256};

fn bytes(value: &[u8]) -> Vec<u8> {
    let mut encoded = Vec::new();
    encode_bytes(&mut encoded, value);
    encoded
}
fn dict(entries: &[(&[u8], Vec<u8>)]) -> Vec<u8> {
    let entries: std::collections::BTreeMap<_, _> =
        entries.iter().map(|(key, value)| (*key, value)).collect();
    let mut result = vec![b'd'];
    for (key, value) in entries {
        encode_bytes(&mut result, key);
        result.extend_from_slice(value);
    }
    result.push(b'e');
    result
}
fn list(values: &[Vec<u8>]) -> Vec<u8> {
    let mut result = vec![b'l'];
    for value in values {
        result.extend_from_slice(value);
    }
    result.push(b'e');
    result
}

#[test]
fn native_metadata_keeps_original_info_trackers_and_web_seeds_without_progress() {
    let info = dict(&[
        (b"name", bytes(b"fixture")),
        (b"length", b"i1e".to_vec()),
        (b"piece length", b"i16384e".to_vec()),
        (b"pieces", bytes(&[1; 20])),
    ]);
    let trackers = list(&[list(&[bytes(b"http://tracker.invalid/announce")])]);
    let webseeds = list(&[bytes(b"http://source.invalid/")]);
    let resume = dict(&[
        (b"info", info.clone()),
        (b"trackers", trackers.clone()),
        (b"url-list", webseeds.clone()),
        (b"httpseeds", webseeds.clone()),
        (b"pieces", bytes(b"resume-progress")),
        (b"save_path", bytes(b"/old/path")),
    ]);
    let result = torrent_from_resume(&resume).unwrap();
    let torrent = parse(&result).unwrap();
    assert_eq!(torrent.get(b"info").unwrap().raw, info);
    assert_eq!(torrent.get(b"announce-list").unwrap().raw, trackers);
    assert_eq!(torrent.get(b"url-list").unwrap().raw, webseeds);
    assert_eq!(torrent.get(b"httpseeds").unwrap().raw, webseeds);
    assert!(torrent.get(b"pieces").is_err());
    assert!(torrent.get(b"save_path").is_err());
}

#[test]
fn native_v2_and_hybrid_restore_full_sparse_and_compact_piece_layers() {
    let leaves: Vec<Vec<u8>> = (1..=3).map(|i| Sha256::digest([i]).to_vec()).collect();
    let left = Sha256::digest([leaves[0].as_slice(), leaves[1].as_slice()].concat()).to_vec();
    let right = Sha256::digest([leaves[2].as_slice(), &[0; 32]].concat()).to_vec();
    let root = Sha256::digest([left.as_slice(), right.as_slice()].concat()).to_vec();
    let layer = leaves.concat();
    for hybrid in [false, true] {
        let file = dict(&[(
            b"",
            dict(&[
                (b"length", b"i40000e".to_vec()),
                (b"pieces root", bytes(&root)),
            ]),
        )]);
        let mut entries = vec![
            (b"name".as_slice(), bytes(b"fixture")),
            (b"meta version", b"i2e".to_vec()),
            (b"piece length", b"i16384e".to_vec()),
            (b"file tree", dict(&[(b"file.bin", file)])),
        ];
        if hybrid {
            entries.push((b"pieces", bytes(&[7; 60])));
        }
        let info = dict(&entries);
        let full = [
            root.as_slice(),
            left.as_slice(),
            right.as_slice(),
            layer.as_slice(),
            &[0; 32],
        ]
        .concat();
        let cases = [
            dict(&[(b"hashes", bytes(&full))]),
            dict(&[(b"hashes", bytes(&layer)), (b"mask", bytes(b"0001110"))]),
            dict(&[(b"hashes", bytes(&layer)), (b"mask", bytes(&[0x1c]))]),
            dict(&[(b"hashes", bytes(&layer))]),
        ];
        for tree in cases {
            let resume = dict(&[(b"info", info.clone()), (b"trees", list(&[tree]))]);
            let encoded = torrent_from_resume(&resume).unwrap();
            let torrent = parse(&encoded).unwrap();
            assert_eq!(torrent.get(b"info").unwrap().raw, info);
            assert_eq!(
                torrent
                    .get(b"piece layers")
                    .unwrap()
                    .get(&root)
                    .unwrap()
                    .bytes()
                    .unwrap(),
                layer
            );
        }
        for hashes in [Vec::new(), vec![0; 96], vec![1; 96]] {
            let resume = dict(&[
                (b"info", info.clone()),
                (b"trees", list(&[dict(&[(b"hashes", bytes(&hashes))])])),
            ]);
            assert!(torrent_from_resume(&resume).is_err());
        }
    }
}

#[test]
fn metadata_parser_rejects_malformed_duplicate_deep_and_trailing_data() {
    for data in [
        b"d4:info".as_slice(),
        b"d1:ai1e1:ai2ee",
        b"i01e",
        b"i-0e",
        b"01:xe",
        b"degarbage",
        b"9999999999999999999999999:x",
    ] {
        assert!(parse(data).is_err(), "{data:?}");
    }
    let deep = [vec![b'l'; 66], vec![b'e'; 66]].concat();
    assert!(parse(&deep).is_err());
    assert!(parse(&list(&vec![b"i0e".to_vec(); 100_001])).is_err());
}

#[test]
fn magnet_identity_accepts_hex_base32_and_v2_but_rejects_ambiguous_hashes() {
    assert_eq!(
        magnet_hashes(&format!("magnet:?xt=urn:btih:{}", "A".repeat(40)))
            .unwrap()
            .0,
        Some("a".repeat(40))
    );
    assert_eq!(
        magnet_hashes(&format!("magnet:?xt=urn:btih:{}", "A".repeat(32)))
            .unwrap()
            .0,
        Some("0".repeat(40))
    );
    assert_eq!(
        magnet_hashes(&format!("magnet:?xt=urn:btmh:1220{}", "a".repeat(64)))
            .unwrap()
            .1,
        Some("a".repeat(64))
    );
    assert!(magnet_hashes("magnet:?xt=urn:btih:invalid").is_err());
    assert!(magnet_hashes(&format!(
        "magnet:?xt=urn:btih:{}&xt=urn:btih:{}",
        "a".repeat(40),
        "b".repeat(40)
    ))
    .is_err());
}

#[test]
fn private_metadata_read_refuses_symlinks_and_non_regular_files() {
    let root = std::env::temp_dir().join(format!(
        "motrix-native-read-{}",
        crate::aria2::generate_rpc_secret()
    ));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("file"), b"data").unwrap();
    std::os::unix::fs::symlink(root.join("file"), root.join("link")).unwrap();
    assert!(read_private_file(&root.join("link")).is_err());
    assert!(read_private_file(&root).is_err());
    fs::remove_dir_all(root).unwrap();
}
