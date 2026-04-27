use crate::model::{ExplorerNode, ImageEntry};
use std::cmp::Ordering;
use std::fs;
use std::io;
use std::path::Path;

const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "gif", "bmp", "tif", "tiff", "heic", "avif",
];

const ARCHIVE_EXTENSIONS: &[&str] = &["zip", "cbz", "rar", "cbr"];

pub fn is_supported_image(path: &Path) -> bool {
    extension_of(path)
        .map(|ext| IMAGE_EXTENSIONS.contains(&ext.as_str()))
        .unwrap_or(false)
}

pub fn is_supported_archive(path: &Path) -> bool {
    extension_of(path)
        .map(|ext| ARCHIVE_EXTENSIONS.contains(&ext.as_str()))
        .unwrap_or(false)
}

pub fn scan_explorer_nodes(directory: &Path) -> io::Result<Vec<ExplorerNode>> {
    let mut nodes = Vec::new();

    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if should_ignore(&path) {
            continue;
        }

        let metadata = entry.metadata()?;
        let is_directory = metadata.is_dir();
        let is_supported_file = is_supported_image(&path) || is_supported_archive(&path);

        if is_directory || is_supported_file {
            nodes.push(ExplorerNode {
                path,
                name: entry.file_name().to_string_lossy().to_string(),
                is_directory,
                is_supported_file,
            });
        }
    }

    nodes.sort_by(sort_node);
    Ok(nodes)
}

pub fn scan_image_entries(directory: &Path, recursive: bool) -> io::Result<Vec<ImageEntry>> {
    let mut images = Vec::new();
    if recursive {
        scan_recursive(directory, &mut images)?;
    } else {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            if should_ignore(&path) {
                continue;
            }
            if entry.metadata()?.is_file() && is_supported_image(&path) {
                images.push(ImageEntry {
                    index: 0,
                    path,
                    name: entry.file_name().to_string_lossy().to_string(),
                });
            }
        }
    }

    images.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    for (index, image) in images.iter_mut().enumerate() {
        image.index = index;
    }
    Ok(images)
}

fn scan_recursive(directory: &Path, images: &mut Vec<ImageEntry>) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if should_ignore(&path) {
            continue;
        }
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            scan_recursive(&path, images)?;
        } else if metadata.is_file() && is_supported_image(&path) {
            images.push(ImageEntry {
                index: 0,
                path: path.clone(),
                name: entry.file_name().to_string_lossy().to_string(),
            });
        }
    }
    Ok(())
}

fn should_ignore(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with('.'))
        .unwrap_or(false)
}

fn extension_of(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
}

fn sort_node(a: &ExplorerNode, b: &ExplorerNode) -> Ordering {
    match (a.is_directory, b.is_directory) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => natural_cmp(&a.name, &b.name),
    }
}

fn natural_cmp(a: &str, b: &str) -> Ordering {
    a.to_lowercase().cmp(&b.to_lowercase())
}
