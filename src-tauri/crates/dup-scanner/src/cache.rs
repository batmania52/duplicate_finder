use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CacheEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vhash_frames: Option<Vec<u64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CacheMeta {
    #[serde(default)]
    pub scan_paths: Vec<String>,
}

/// 파일 저장 포맷 (래퍼)
#[derive(Serialize, Deserialize)]
struct CacheFile {
    #[serde(default)]
    meta: CacheMeta,
    #[serde(default)]
    entries: HashMap<String, CacheEntry>,
}

pub struct HashCache {
    pub meta: CacheMeta,
    entries: HashMap<String, CacheEntry>,
    dirty_count: usize,
    last_flush: Instant,
    path: PathBuf,
}

const FLUSH_DIRTY_THRESHOLD: usize = 100;
const FLUSH_INTERVAL: Duration = Duration::from_secs(30);

impl HashCache {
    /// 파일 없이 빈 캐시 생성 (경로만 지정).
    pub fn empty(path: PathBuf) -> Self {
        Self {
            meta: CacheMeta::default(),
            entries: HashMap::new(),
            dirty_count: 0,
            last_flush: Instant::now(),
            path,
        }
    }

    /// 파일에서 캐시 로드. 파싱 오류·구버전 포맷 시 빈 캐시 반환.
    pub fn load(path: &Path) -> Result<Self> {
        let (meta, entries) = if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(s) => {
                    // 신규 포맷: 최상위에 "entries" 키가 있는 경우
                    let v: serde_json::Value = serde_json::from_str(&s).unwrap_or_default();
                    if v.get("entries").is_some() {
                        if let Ok(f) = serde_json::from_value::<CacheFile>(v) {
                            (f.meta, f.entries)
                        } else {
                            (CacheMeta::default(), HashMap::new())
                        }
                    } else {
                        // 구버전 포맷 (HashMap 직렬화) 폴백
                        let entries: HashMap<String, CacheEntry> =
                            serde_json::from_str(&s).unwrap_or_default();
                        (CacheMeta::default(), entries)
                    }
                }
                Err(_) => (CacheMeta::default(), HashMap::new()),
            }
        } else {
            (CacheMeta::default(), HashMap::new())
        };

        Ok(Self {
            meta,
            entries,
            dirty_count: 0,
            last_flush: Instant::now(),
            path: path.to_path_buf(),
        })
    }

    pub fn get(&self, key: &str) -> Option<&CacheEntry> {
        self.entries.get(key)
    }

    pub fn insert(&mut self, key: String, entry: CacheEntry) {
        self.entries.insert(key, entry);
        self.dirty_count += 1;
    }

    /// 배치 결과를 캐시에 병합. par_iter 완료 후 호출.
    pub fn merge_batch(&mut self, batch: Vec<(String, CacheEntry)>) {
        let count = batch.len();
        for (key, entry) in batch {
            self.entries.insert(key, entry);
        }
        self.dirty_count += count;
    }

    /// dirty_count >= 100 또는 30초 경과 시 flush.
    pub fn flush_if_needed(&mut self) -> Result<()> {
        if self.dirty_count >= FLUSH_DIRTY_THRESHOLD
            || self.last_flush.elapsed() >= FLUSH_INTERVAL
        {
            self.flush()?;
        }
        Ok(())
    }

    /// atomic write: path.tmp 저장 후 rename.
    pub fn flush(&mut self) -> Result<()> {
        let tmp = self.path.with_extension("json.tmp");
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = CacheFile {
            meta: self.meta.clone(),
            entries: self.entries.clone(),
        };
        let data = serde_json::to_string(&file)?;
        std::fs::write(&tmp, data)?;
        std::fs::rename(&tmp, &self.path)?;
        self.dirty_count = 0;
        self.last_flush = Instant::now();
        Ok(())
    }

    /// "path|size|mtime" 형식의 캐시 키 생성.
    pub fn cache_key(path: &Path) -> Option<String> {
        let meta = std::fs::metadata(path).ok()?;
        let mtime = meta
            .modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs();
        Some(format!("{}|{}|{}", path.display(), meta.len(), mtime))
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_cache_key_format() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"hello").unwrap();
        let key = HashCache::cache_key(f.path()).unwrap();
        let parts: Vec<&str> = key.split('|').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[1], "5");
    }

    #[test]
    fn test_flush_and_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.json");

        let mut cache = HashCache::load(&path).unwrap();
        cache.meta.scan_paths = vec!["/foo/bar".to_string()];
        cache.insert("key1".to_string(), CacheEntry {
            hash: Some("abc".to_string()), phash: None, vhash_frames: None,
        });
        cache.flush().unwrap();

        let reloaded = HashCache::load(&path).unwrap();
        assert_eq!(reloaded.get("key1").unwrap().hash.as_deref(), Some("abc"));
        assert_eq!(reloaded.meta.scan_paths, vec!["/foo/bar"]);
    }

    #[test]
    fn test_legacy_format_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.json");
        // 구버전 포맷 (HashMap 직렬화)
        let legacy = serde_json::json!({
            "key1": { "hash": "abc" }
        });
        std::fs::write(&path, legacy.to_string()).unwrap();

        let cache = HashCache::load(&path).unwrap();
        assert_eq!(cache.get("key1").unwrap().hash.as_deref(), Some("abc"));
        assert!(cache.meta.scan_paths.is_empty());
    }

    #[test]
    fn test_merge_batch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.json");
        let mut cache = HashCache::load(&path).unwrap();
        let batch = vec![
            ("k1".to_string(), CacheEntry { hash: Some("h1".to_string()), phash: None, vhash_frames: None }),
            ("k2".to_string(), CacheEntry { hash: Some("h2".to_string()), phash: None, vhash_frames: None }),
        ];
        cache.merge_batch(batch);
        assert_eq!(cache.dirty_count, 2);
        assert!(cache.get("k1").is_some());
    }

    #[test]
    fn test_load_missing_file_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        let cache = HashCache::load(&path).unwrap();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_corrupt_cache_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.json");
        std::fs::write(&path, b"not-valid-json{{").unwrap();
        let cache = HashCache::load(&path).unwrap();
        assert!(cache.is_empty());
    }
}
