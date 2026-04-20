use std::path::{Path, PathBuf};
use globset::{Glob, GlobSet, GlobSetBuilder};
use walkdir::WalkDir;

const IGNORE_NAMES: &[&str] = &[
    ".DS_Store", ".Spotlight-V100", ".Trashes", ".fseventsd",
    "Thumbs.db", "desktop.ini",
];
const IGNORE_EXTENSIONS: &[&str] = &[".tmp", ".temp", ".part"];

pub fn build_exclude_set(patterns: &[String]) -> anyhow::Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for p in patterns {
        builder.add(Glob::new(p)?);
    }
    Ok(builder.build()?)
}

pub fn collect_files(paths: &[String], exclude_patterns: &[String]) -> anyhow::Result<Vec<PathBuf>> {
    let exclude_set = build_exclude_set(exclude_patterns)?;
    let mut files = Vec::new();

    for root in paths {
        for entry in WalkDir::new(root).follow_links(false).into_iter().flatten() {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if should_ignore(path, &exclude_set) {
                continue;
            }
            files.push(path.to_path_buf());
        }
    }

    Ok(files)
}

fn should_ignore(path: &Path, exclude_set: &GlobSet) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if IGNORE_NAMES.contains(&name) {
        return true;
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let ext_with_dot = format!(".{}", ext.to_lowercase());
    if IGNORE_EXTENSIONS.contains(&ext_with_dot.as_str()) {
        return true;
    }
    let path_str = path.to_string_lossy();
    if exclude_set.is_match(path_str.as_ref()) {
        return true;
    }
    // 파일명 기준 exclude 패턴도 검사
    if exclude_set.is_match(name) {
        return true;
    }
    false
}

pub const IMAGE_EXTENSIONS: &[&str] = &[
    ".jpg", ".jpeg", ".png", ".gif", ".bmp", ".webp", ".tiff", ".tif", ".heic", ".heif",
];

pub const VIDEO_EXTENSIONS: &[&str] = &[
    ".mp4", ".mkv", ".avi", ".mov", ".wmv", ".flv", ".webm",
    ".m4v", ".mpg", ".mpeg", ".ts", ".mts", ".m2ts",
];

pub const ARCHIVE_EXTENSIONS: &[&str] = &[
    ".zip", ".7z", ".rar",
];

pub fn is_image(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    IMAGE_EXTENSIONS.contains(&format!(".{}", ext.to_lowercase()).as_str())
}

pub fn is_video(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    VIDEO_EXTENSIONS.contains(&format!(".{}", ext.to_lowercase()).as_str())
}

pub fn is_archive(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    ARCHIVE_EXTENSIONS.contains(&format!(".{}", ext.to_lowercase()).as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exclude_pattern() {
        let exclude = vec!["*.tmp".to_string(), ".DS_Store".to_string()];
        let set = build_exclude_set(&exclude).unwrap();
        assert!(set.is_match("foo.tmp"));
        assert!(!set.is_match("foo.txt"));
    }

    #[test]
    fn test_is_image() {
        assert!(is_image(Path::new("photo.jpg")));
        assert!(is_image(Path::new("photo.JPEG")));
        assert!(!is_image(Path::new("video.mp4")));
    }
}
