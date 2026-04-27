use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone)]
pub struct PrefetchWindow {
    pub backward: usize,
    pub forward: usize,
    pub max_concurrent_jobs: usize,
}

#[derive(Debug, Clone)]
pub struct CachePolicy {
    pub memory_limit_bytes: u64,
    pub disk_limit_bytes: u64,
    pub full_cache_threshold_bytes: u64,
    pub prefetch_window: PrefetchWindow,
}

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub key: String,
    pub source_path: String,
    pub source_name: String,
    pub image_index: usize,
    pub image_path: String,
    pub last_accessed_at_ms: u64,
}

#[derive(Debug, Default)]
pub struct CacheState {
    order: VecDeque<String>,
    entries: HashMap<String, CacheEntry>,
}

impl CacheState {
    pub fn insert(&mut self, entry: CacheEntry) {
        let key = entry.key.clone();
        if self.entries.contains_key(&key) {
            self.order.retain(|existing| existing != &key);
        }
        self.order.push_back(key.clone());
        self.entries.insert(key, entry);
    }

    pub fn get(&mut self, key: &str) -> Option<&CacheEntry> {
        if self.entries.contains_key(key) {
            self.order.retain(|existing| existing != key);
            self.order.push_back(key.to_string());
        }
        self.entries.get(key)
    }

    pub fn remove_oldest(&mut self) -> Option<CacheEntry> {
        let oldest = self.order.pop_front()?;
        self.entries.remove(&oldest)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}
