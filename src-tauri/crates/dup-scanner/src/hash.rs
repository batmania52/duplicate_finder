use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
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

pub fn hash_bytes(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

fn file_created(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    let created = meta.created().or_else(|_| meta.modified()).ok()?;
    let dt: DateTime<chrono::Local> = created.into();
    Some(dt.format("%Y-%m-%dT%H:%M:%S").to_string())
}

pub fn find_duplicates(files: &[std::path::PathBuf], log_tx: Option<&crate::LogSender>, cancel: Option<&tokio_util::sync::CancellationToken>) -> Vec<Group> {
    // 크기 기준 1차 필터 (같은 크기끼리만 해시)
    let mut by_size: HashMap<u64, Vec<&std::path::PathBuf>> = HashMap::new();
    for f in files {
        if let Ok(meta) = std::fs::metadata(f) {
            by_size.entry(meta.len()).or_default().push(f);
        }
    }

    let candidates: Vec<&std::path::PathBuf> = by_size
        .values()
        .filter(|v| v.len() > 1)
        .flatten()
        .copied()
        .collect();

    let total = candidates.len();
    let done = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

    // 병렬 해시 계산
    let hashed: Vec<(String, &std::path::PathBuf)> = candidates
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
    use std::io::Write;
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

        let groups = find_duplicates(&[p1, p2, p3]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 2);
    }
}
