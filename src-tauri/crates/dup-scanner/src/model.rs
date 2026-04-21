use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub size_fmt: String,
    pub keep: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phash: Option<String>,
    #[serde(rename = "type")]
    pub file_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame_count: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_files: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Group {
    pub id: String,
    pub files: Vec<FileEntry>,
    pub savable: u64,
    pub savable_fmt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScanResult {
    pub regular: Vec<Group>,
    pub image: Vec<Group>,
    pub video: Vec<Group>,
    pub archive: Vec<Group>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanOptions {
    pub paths: Vec<String>,
    #[serde(default)]
    pub no_phash: bool,
    #[serde(default)]
    pub no_vhash: bool,
    #[serde(default)]
    pub no_archive: bool,
    #[serde(default = "default_phash_exact")]
    pub phash_exact: u32,
    #[serde(default = "default_phash_similar")]
    pub phash_similar: u32,
    #[serde(default = "default_vhash_frames")]
    pub vhash_frames: u32,
    #[serde(default = "default_vhash_exact")]
    pub vhash_exact: f32,
    #[serde(default = "default_vhash_similar")]
    pub vhash_similar: f32,
    #[serde(default = "default_min_overlap")]
    pub min_overlap: u32,
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
    #[serde(default)]
    pub num_threads: usize,
    #[serde(default)]
    pub min_size_kb: u64,
    #[serde(default)]
    pub check_inode: bool,
    #[serde(default = "default_partial_hash_kb")]
    pub partial_hash_kb: u64,
}

fn default_partial_hash_kb() -> u64 { 64 }

fn default_phash_exact() -> u32 { 0 }
fn default_phash_similar() -> u32 { 10 }
fn default_vhash_frames() -> u32 { 10 }
fn default_vhash_exact() -> f32 { 3.0 }
fn default_vhash_similar() -> f32 { 10.0 }
fn default_min_overlap() -> u32 { 2 }

pub fn fmt_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    if bytes == 0 {
        return "0 B".to_string();
    }
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[unit_idx])
    } else {
        format!("{:.1} {}", size, UNITS[unit_idx])
    }
}
