use super::bencode::{invalid, Node};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

// libtorrent 2.0 的 fastresume 保存完整或稀疏 Merkle 树，不能只复制 info 丢掉 v2 piece layers。
pub(super) fn piece_layers(
    resume: &Node<'_>,
    info: &Node<'_>,
) -> Result<BTreeMap<Vec<u8>, Vec<u8>>, String> {
    let mut required = BTreeMap::new();
    let piece_length = info.get(b"piece length")?.integer()?;
    if piece_length < 16384 || !piece_length.is_power_of_two() {
        return Err(invalid());
    }
    collect_files(info.get(b"file tree")?, piece_length, &mut required)?;
    if required.is_empty() {
        return Ok(BTreeMap::new());
    }
    let trees = resume.get(b"trees")?.list()?;
    let mut result = BTreeMap::new();
    let mut work_budget = 1_000_000usize;
    for (root, length) in required {
        let pieces = usize::try_from(length.div_ceil(piece_length)).map_err(|_| invalid())?;
        let blocks = usize::try_from(length.div_ceil(16384)).map_err(|_| invalid())?;
        let width = blocks.checked_next_power_of_two().ok_or_else(invalid)?;
        let layer_width = width / usize::try_from(piece_length / 16384).map_err(|_| invalid())?;
        if layer_width == 0 || pieces > 100_000 {
            return Err(invalid());
        }
        let first = layer_width - 1;
        let mut matched = None;
        for tree in trees {
            let hashes = tree.get(b"hashes")?.bytes()?;
            if hashes.len() % 32 != 0 {
                return Err(invalid());
            }
            work_budget = work_budget
                .checked_sub(hashes.len() / 32 + 1)
                .ok_or_else(invalid)?;
            let mask = tree
                .dict()?
                .get(b"mask".as_slice())
                .map(|node| node.bytes())
                .transpose()?;
            let mut layer = Vec::with_capacity(pieces * 32);
            if let Some(mask) = mask {
                let nodes = width
                    .checked_mul(2)
                    .and_then(|size| size.checked_sub(1))
                    .ok_or_else(invalid)?;
                let ascii =
                    mask.len() == nodes && mask.iter().all(|bit| matches!(bit, b'0' | b'1'));
                if !ascii && mask.len() != nodes.div_ceil(8) {
                    continue;
                }
                work_budget = work_budget.checked_sub(nodes).ok_or_else(invalid)?;
                let present = |index: usize| {
                    if ascii {
                        mask[index] == b'1'
                    } else {
                        mask[index / 8] & (0x80 >> (index % 8)) != 0
                    }
                };
                if (0..nodes).filter(|index| present(*index)).count() * 32 != hashes.len() {
                    return Err(invalid());
                }
                let mut offset = (0..first).filter(|index| present(*index)).count() * 32;
                for index in first..first + pieces {
                    if !present(index) {
                        break;
                    }
                    layer.extend_from_slice(&hashes[offset..offset + 32]);
                    offset += 32;
                }
            } else if hashes.len() / 32 == width.saturating_mul(2).saturating_sub(1) {
                layer.extend_from_slice(&hashes[first * 32..(first + pieces) * 32]);
            } else if hashes.len() / 32 == pieces {
                layer.extend_from_slice(hashes);
            }
            if layer.len() != pieces * 32
                || layer
                    .chunks_exact(32)
                    .any(|hash| hash.iter().all(|byte| *byte == 0))
            {
                continue;
            }
            if layer_root(&layer, piece_length)? == root.as_slice() {
                matched = Some(layer);
                break;
            }
        }
        result.insert(root, matched.ok_or_else(invalid)?);
    }
    Ok(result)
}

fn collect_files(
    node: &Node<'_>,
    piece_length: u64,
    files: &mut BTreeMap<Vec<u8>, u64>,
) -> Result<(), String> {
    for (name, child) in node.dict()? {
        if name.is_empty() {
            let length = child.get(b"length")?.integer()?;
            if length > piece_length {
                let root = child.get(b"pieces root")?.bytes()?;
                if root.len() != 32 {
                    return Err(invalid());
                }
                if files
                    .insert(root.to_vec(), length)
                    .is_some_and(|old| old != length)
                {
                    return Err(invalid());
                }
            }
        } else {
            collect_files(child, piece_length, files)?;
        }
    }
    Ok(())
}

fn layer_root(layer: &[u8], piece_length: u64) -> Result<Vec<u8>, String> {
    let mut pad = vec![0u8; 32];
    let mut blocks = piece_length / 16384;
    while blocks > 1 {
        pad = Sha256::digest([pad.as_slice(), pad.as_slice()].concat()).to_vec();
        blocks /= 2;
    }
    let mut hashes: Vec<Vec<u8>> = layer.chunks_exact(32).map(|hash| hash.to_vec()).collect();
    let width = hashes
        .len()
        .checked_next_power_of_two()
        .ok_or_else(invalid)?;
    hashes.resize(width, pad);
    while hashes.len() > 1 {
        hashes = hashes
            .chunks_exact(2)
            .map(|pair| Sha256::digest([pair[0].as_slice(), pair[1].as_slice()].concat()).to_vec())
            .collect();
    }
    hashes.pop().ok_or_else(invalid)
}
