use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use crate::filter::ExcludeFilter;

const IGNORE_NAMES: &[&str] = &[
    ".DS_Store", ".Spotlight-V100", ".Trashes", ".fseventsd",
    "Thumbs.db", "desktop.ini",
];
const IGNORE_EXTENSIONS: &[&str] = &[".tmp", ".temp", ".part"];

pub fn collect_files(paths: &[String], exclude_patterns: &[String]) -> anyhow::Result<Vec<PathBuf>> {
    let filter = ExcludeFilter::from_patterns(exclude_patterns)?;
    let mut files = Vec::new();

    for root in paths {
        let walker = WalkDir::new(root).follow_links(false).into_iter();
        let mut iter = walker.filter_entry(|e| {
            if e.file_type().is_dir() {
                return !filter.should_skip_dir(e.path());
            }
            true
        });
        while let Some(entry) = iter.next() {
            let entry = match entry { Ok(e) => e, Err(_) => continue };
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if should_ignore_builtin(path) || filter.should_skip_file(path) {
                continue;
            }
            files.push(path.to_path_buf());
        }
    }

    Ok(files)
}

fn should_ignore_builtin(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if IGNORE_NAMES.contains(&name) {
        return true;
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let ext_with_dot = format!(".{}", ext.to_lowercase());
    IGNORE_EXTENSIONS.contains(&ext_with_dot.as_str())
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
        use crate::filter::ExcludeFilter;
        let exclude = vec!["*.tmp".to_string()];
        let f = ExcludeFilter::from_patterns(&exclude).unwrap();
        assert!(f.should_skip_file(Path::new("foo.tmp")));
        assert!(!f.should_skip_file(Path::new("foo.txt")));
    }

    #[test]
    fn test_is_image() {
        assert!(is_image(Path::new("photo.jpg")));
        assert!(is_image(Path::new("photo.JPEG")));
        assert!(!is_image(Path::new("video.mp4")));
    }
}
