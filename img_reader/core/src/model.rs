use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Image,
    Folder,
    Archive,
}

#[derive(Debug, Clone)]
pub struct ReaderSession {
    pub id: String,
    pub source_path: PathBuf,
    pub source_name: String,
    pub source_kind: SourceKind,
    pub current_index: usize,
    pub total_count: usize,
}

#[derive(Debug, Clone)]
pub struct ImageEntry {
    pub index: usize,
    pub path: PathBuf,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct ExplorerNode {
    pub path: PathBuf,
    pub name: String,
    pub is_directory: bool,
    pub is_supported_file: bool,
}
