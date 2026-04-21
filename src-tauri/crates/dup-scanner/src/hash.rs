use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use sha2::{Digest, Sha256};
use chrono::DateTime;
use rayon::prelude::*;
use crate::model::{FileEntry, Group, fmt_size};

const CHUNK_SIZE: usize = 65536;

pub fn hash_file(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK_SIZE];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buf[..n]),
            Err(_) => return None,
        }
    }
    Some(hex::encode(hasher.finalize()))
}

/// 파일 앞 N 바이트만 해시 (부분 해시 1단계)
fn partial_hash_head(path: &Path, limit_bytes: u64) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut remaining = limit_bytes as usize;
    while remaining > 0 {
        let to_read = remaining.min(CHUNK_SIZE);
        match reader.read(&mut buf[..to_read]) {
            Ok(0) => break,
            Ok(n) => { hasher.update(&buf[..n]); remaining -= n; }
            Err(_) => return None,
        }
    }
    Some(hex::encode(hasher.finalize()))
}

/// 파일 뒤 N 바이트만 해시 (부분 해시 2단계)
fn partial_hash_tail(path: &Path, limit_bytes: u64) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let file_size = file.metadata().ok()?.len();
    let offset = file_size.saturating_sub(limit_bytes);
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK_SIZE];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buf[..n]),
            Err(_) => return None,
        }
    }
    Some(hex::encode(hasher.finalize()))
}

pub fn hash_bytes(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

fn file_created(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    let created = meta.created().or_else(|_| meta.modified()).ok()?;
    let dt: DateTime<chrono::Local> = created.into();
    Some(dt.format("%Y-%m-%dT%H:%M:%S").to_string())
}

#[cfg(unix)]
fn get_inode(path: &Path) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path).ok().map(|m| m.ino())
}

pub fn find_duplicates(files: &[std::path::PathBuf], log_tx: Option<&crate::LogSender>, cancel: Option<&tokio_util::sync::CancellationToken>) -> Vec<Group> {
    find_duplicates_opts(files, log_tx, cancel, 0, false, 64)
}

pub fn find_duplicates_opts(
    files: &[std::path::PathBuf],
    log_tx: Option<&crate::LogSender>,
    cancel: Option<&tokio_util::sync::CancellationToken>,
    min_size_kb: u64,
    check_inode: bool,
    partial_hash_kb: u64,
) -> Vec<Group> {
    let min_bytes = min_size_kb * 1024;

    // 크기 기준 1차 필터 (같은 크기끼리만 해시)
    let mut by_size: HashMap<u64, Vec<&std::path::PathBuf>> = HashMap::new();
    for f in files {
        if let Ok(meta) = std::fs::metadata(f) {
            let len = meta.len();
            if min_bytes > 0 && len < min_bytes {
                continue;
            }
            by_size.entry(len).or_default().push(f);
        }
    }

    // inode 하드링크 사전 그룹화 (Unix only)
    #[cfg(unix)]
    let (inode_groups, remaining): (Vec<Group>, Vec<&std::path::PathBuf>) = if check_inode {
        let mut by_inode: HashMap<(u64, u64), Vec<&std::path::PathBuf>> = HashMap::new();
        let mut no_inode: Vec<&std::path::PathBuf> = Vec::new();
        for size_group in by_size.values().filter(|v| v.len() > 1) {
            for p in size_group {
                let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
                if let Some(ino) = get_inode(p) {
                    by_inode.entry((ino, size)).or_default().push(p);
                } else {
                    no_inode.push(p);
                }
            }
        }
        let groups: Vec<Group> = by_inode
            .into_iter()
            .filter(|(_, v)| v.len() > 1)
            .enumerate()
            .map(|(i, (_, paths))| {
                let dummy_hash = format!("inode-{}", i);
                let files: Vec<FileEntry> = paths.iter().enumerate().map(|(fi, p)| {
                    let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
                    FileEntry {
                        path: p.to_string_lossy().to_string(),
                        size,
                        size_fmt: fmt_size(size),
                        keep: fi == 0,
                        hash: Some(dummy_hash.clone()),
                        file_type: "file".to_string(),
                        created: file_created(p),
                        ..Default::default()
                    }
                }).collect();
                let savable: u64 = files.iter().skip(1).map(|f| f.size).sum();
                Group {
                    id: format!("r-ino{}", i + 1),
                    savable,
                    savable_fmt: fmt_size(savable),
                    files,
                    ..Default::default()
                }
            })
            .collect();
        // inode 그룹에 속하지 않은 파일만 일반 해시 후보로
        let remaining: Vec<&std::path::PathBuf> = no_inode;
        (groups, remaining)
    } else {
        let all: Vec<&std::path::PathBuf> = by_size
            .values()
            .filter(|v| v.len() > 1)
            .flatten()
            .copied()
            .collect();
        (Vec::new(), all)
    };

    #[cfg(not(unix))]
    let (inode_groups, remaining): (Vec<Group>, Vec<&std::path::PathBuf>) = {
        let all: Vec<&std::path::PathBuf> = by_size
            .values()
            .filter(|v| v.len() > 1)
            .flatten()
            .copied()
            .collect();
        (Vec::new(), all)
    };

    let candidates: Vec<&std::path::PathBuf> = if check_inode {
        remaining
    } else {
        by_size.values().filter(|v| v.len() > 1).flatten().copied().collect()
    };

    let limit = partial_hash_kb * 1024;
    let total = candidates.len();
    let done = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

    // 3단계 파이프라인 (partial_hash_kb == 0이면 단일 full_hash)
    let final_candidates: Vec<&std::path::PathBuf> = if limit > 0 {
        // 1단계: 앞 N KB 해시로 필터
        let head_hashed: Vec<(String, &std::path::PathBuf)> = candidates
            .par_iter()
            .filter_map(|p| {
                if cancel.map(|c| c.is_cancelled()).unwrap_or(false) { return None; }
                partial_hash_head(p, limit).map(|h| (h, *p))
            })
            .collect();

        let mut by_head: HashMap<String, Vec<&std::path::PathBuf>> = HashMap::new();
        for (h, p) in &head_hashed {
            by_head.entry(h.clone()).or_default().push(p);
        }
        let after_head: Vec<&std::path::PathBuf> = by_head
            .into_values()
            .filter(|v| v.len() > 1)
            .flatten()
            .collect();

        // 2단계: 뒤 N KB 해시로 필터
        let tail_hashed: Vec<(String, &std::path::PathBuf)> = after_head
            .par_iter()
            .filter_map(|p| {
                if cancel.map(|c| c.is_cancelled()).unwrap_or(false) { return None; }
                partial_hash_tail(p, limit).map(|h| (h, *p))
            })
            .collect();

        let mut by_tail: HashMap<String, Vec<&std::path::PathBuf>> = HashMap::new();
        for (h, p) in &tail_hashed {
            by_tail.entry(h.clone()).or_default().push(p);
        }
        by_tail.into_values().filter(|v| v.len() > 1).flatten().collect()
    } else {
        candidates.iter().copied().collect()
    };

    // 3단계 (또는 단일): full SHA-256
    let hashed: Vec<(String, &std::path::PathBuf)> = final_candidates
        .par_iter()
        .filter_map(|p| {
            if cancel.map(|c| c.is_cancelled()).unwrap_or(false) { return None; }
            let result = hash_file(p).map(|h| (h, *p));
            let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            if let Some(tx) = log_tx {
                if n % 200 == 0 || n == total {
                    let _ = tx.send(format!("\r해시 중복 탐지 중... ({} / {})", n, total));
                }
            }
            result
        })
        .collect();

    // 해시 기준 그룹화
    let mut by_hash: HashMap<String, Vec<&std::path::PathBuf>> = HashMap::new();
    for (h, p) in &hashed {
        by_hash.entry(h.clone()).or_default().push(p);
    }

    let mut groups: Vec<Group> = by_hash
        .into_iter()
        .filter(|(_, v)| v.len() > 1)
        .enumerate()
        .map(|(i, (hash, paths))| {
            let files: Vec<FileEntry> = paths.iter().map(|p| {
                let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
                FileEntry {
                    path: p.to_string_lossy().to_string(),
                    size,
                    size_fmt: fmt_size(size),
                    keep: false,
                    hash: Some(hash.clone()),
                    file_type: "file".to_string(),
                    created: file_created(p),
                    ..Default::default()
                }
            }).collect();

            let savable: u64 = files.iter().skip(1).map(|f| f.size).sum();
            Group {
                id: format!("r{}", i + 1),
                savable,
                savable_fmt: fmt_size(savable),
                files,
                ..Default::default()
            }
        })
        .collect();

    // inode 그룹 병합
    groups.extend(inode_groups);

    // 절약 용량 내림차순 정렬
    groups.sort_by(|a, b| b.savable.cmp(&a.savable));
    // 각 그룹 첫 파일 keep=true
    for g in &mut groups {
        if let Some(first) = g.files.first_mut() {
            first.keep = true;
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::tempdir;

    #[test]
    fn test_hash_bytes() {
        let h1 = hash_bytes(b"hello");
        let h2 = hash_bytes(b"hello");
        let h3 = hash_bytes(b"world");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_find_duplicates() {
        let dir = tempdir().unwrap();
        let p1 = dir.path().join("a.txt");
        let p2 = dir.path().join("b.txt");
        let p3 = dir.path().join("c.txt");
        std::fs::write(&p1, b"same content").unwrap();
        std::fs::write(&p2, b"same content").unwrap();
        std::fs::write(&p3, b"different").unwrap();

        let groups = find_duplicates(&[p1, p2, p3], None, None);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 2);
    }
}
