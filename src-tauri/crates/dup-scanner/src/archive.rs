use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use crate::hash::hash_bytes;
use crate::model::{FileEntry, Group, fmt_size};

pub type LogSender = tokio::sync::mpsc::UnboundedSender<String>;

pub fn find_archive_duplicates(
    files: &[std::path::PathBuf],
    min_overlap: u32,
    log_tx: Option<&LogSender>,
) -> Vec<Group> {
    let total = files.len();
    let mut archive_entries: Vec<(String, HashMap<String, u64>)> = Vec::new();

    for (i, path) in files.iter().enumerate() {
        if let Some(tx) = log_tx {
            let _ = tx.send(format!("\r아카이브 분석 중... ({}/{})", i + 1, total));
        }
        if let Some(entries) = extract_entries(path) {
            archive_entries.push((path.to_string_lossy().to_string(), entries));
        }
    }

    if archive_entries.len() < 2 {
        return Vec::new();
    }

    let mut groups: Vec<Group> = Vec::new();
    let mut group_id = 1;

    for i in 0..archive_entries.len() {
        for j in (i + 1)..archive_entries.len() {
            let (path_a, entries_a) = &archive_entries[i];
            let (path_b, entries_b) = &archive_entries[j];

            let shared_count = entries_a
                .values()
                .filter(|h| entries_b.values().any(|h2| h2 == *h))
                .count() as u32;

            if shared_count >= min_overlap {
                let size_a = std::fs::metadata(path_a).map(|m| m.len()).unwrap_or(0);
                let size_b = std::fs::metadata(path_b).map(|m| m.len()).unwrap_or(0);
                let savable = size_a.min(size_b);

                let total_a = entries_a.len() as u32;
                let total_b = entries_b.len() as u32;
                let files = vec![
                    FileEntry {
                        path: path_a.clone(),
                        size: size_a,
                        size_fmt: fmt_size(size_a),
                        keep: true,
                        file_type: "archive".to_string(),
                        shared: Some(shared_count),
                        total_files: Some(total_a),
                        ..Default::default()
                    },
                    FileEntry {
                        path: path_b.clone(),
                        size: size_b,
                        size_fmt: fmt_size(size_b),
                        keep: false,
                        file_type: "archive".to_string(),
                        shared: Some(shared_count),
                        total_files: Some(total_b),
                        ..Default::default()
                    },
                ];

                groups.push(Group {
                    id: format!("a{}", group_id),
                    shared: Some(shared_count),
                    savable,
                    savable_fmt: fmt_size(savable),
                    files,
                    ..Default::default()
                });
                group_id += 1;
            }
        }
    }

    groups.sort_by(|a, b| b.savable.cmp(&a.savable));
    groups
}

fn extract_entries(path: &Path) -> Option<HashMap<String, u64>> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    match ext.as_str() {
        "zip" => extract_zip(path),
        "7z" => extract_7z(path),
        _ => None,
    }
}

fn extract_zip(path: &Path) -> Option<HashMap<String, u64>> {
    let file = File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let mut entries = HashMap::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).ok()?;
        if entry.is_dir() { continue; }
        let name = entry.name().to_string();
        let size = entry.size();
        if size == 0 { continue; }

        let hash = if size <= 1024 * 1024 {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).ok()?;
            u64::from_str_radix(&hash_bytes(&buf)[..16], 16).unwrap_or(size)
        } else {
            size ^ (name.len() as u64)
        };
        entries.insert(name, hash);
    }
    Some(entries)
}

fn extract_7z(path: &Path) -> Option<HashMap<String, u64>> {
    let mut entries = HashMap::new();
    sevenz_rust::decompress_file_with_extract_fn(path, std::path::Path::new("/dev/null"), |entry, reader, _dest| {
        if entry.is_directory() { return Ok(true); }
        let name = entry.name().to_string();
        let size = entry.size();
        if size == 0 { return Ok(true); }

        if size <= 1024 * 1024 {
            let mut buf = Vec::new();
            if reader.read_to_end(&mut buf).is_ok() {
                let hash = u64::from_str_radix(&hash_bytes(&buf)[..16], 16).unwrap_or(size);
                entries.insert(name, hash);
            }
        } else {
            let hash = size ^ (name.len() as u64);
            entries.insert(name, hash);
        }
        Ok(true)
    }).ok()?;
    Some(entries)
}
