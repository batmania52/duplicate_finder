pub mod model;
pub mod collector;
pub mod filter;
pub mod hash;
pub mod phash;
pub mod vhash;
pub mod archive;
pub mod cache;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;
use crate::cache::HashCache;
use crate::collector::{collect_files, is_image, is_video, is_archive};
use crate::model::{ScanOptions, ScanResult};

pub type LogSender = tokio::sync::mpsc::UnboundedSender<String>;

/// cache_path 지정 시 그대로 사용, 미지정 시 문서 디렉토리에 타임스탬프 파일명 생성.
fn resolve_cache_path(cache_path: Option<&str>) -> Option<std::path::PathBuf> {
    if let Some(p) = cache_path {
        return Some(std::path::PathBuf::from(p));
    }
    let docs = docs_dir()?;
    let ts = chrono::Local::now().format("%Y%m%d%H%M%S");
    Some(docs.join(format!("dup-cache-{}.json", ts)))
}

/// OS별 문서 디렉토리 반환.
fn docs_dir() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").ok()?;
        return Some(std::path::PathBuf::from(home).join("Documents"));
    }
    #[cfg(target_os = "windows")]
    {
        let userprofile = std::env::var("USERPROFILE").ok()?;
        return Some(std::path::PathBuf::from(userprofile).join("Documents"));
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let home = std::env::var("HOME").ok()?;
        return Some(std::path::PathBuf::from(home).join("Documents"));
    }
}

fn is_same_zip_only_group(group: &crate::model::Group) -> bool {
    if group.files.is_empty() { return false; }
    if !group.files.iter().all(|f| f.path.contains("::")) { return false; }
    let first_zip = group.files[0].path.split("::").next().unwrap_or("");
    group.files.iter().all(|f| f.path.split("::").next().unwrap_or("") == first_zip)
}

pub async fn run_scan(
    options: ScanOptions,
    log_tx: LogSender,
    cancel: CancellationToken,
) -> anyhow::Result<ScanResult> {
    let num_threads = if options.num_threads > 0 {
        options.num_threads
    } else {
        std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
    };
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build_global()
        .ok();
    let _ = log_tx.send(format!("파일 목록 수집 중... (스레드 {}개)", num_threads));

    let files = tokio::task::spawn_blocking({
        let paths = options.paths.clone();
        let excludes = options.exclude_patterns.clone();
        move || collect_files(&paths, &excludes)
    }).await??;

    if cancel.is_cancelled() {
        return Ok(ScanResult::default());
    }

    let _ = log_tx.send(format!("총 {} 파일 수집 완료", files.len()));

    // 캐시 초기화 — cache_auto_save=true(기본)이면 항상 활성화
    // cache_path 미지정 시 문서 디렉토리에 타임스탬프 파일 자동 생성
    let cache: Option<Arc<Mutex<HashCache>>> = if options.cache_auto_save {
        let path = resolve_cache_path(options.cache_path.as_deref());
        match path {
            Some(p) => {
                let mut loaded = HashCache::load(&p).unwrap_or_else(|_| HashCache::empty(p.clone()));
                loaded.meta.scan_paths = options.paths.clone();
                let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let _ = log_tx.send(format!("캐시: {}", fname));
                Some(Arc::new(Mutex::new(loaded)))
            }
            None => {
                let _ = log_tx.send("캐시 경로 결정 실패 — 캐시 비활성".to_string());
                None
            }
        }
    } else {
        None
    };

    // 파일 분류
    let mut regular_files: Vec<PathBuf> = Vec::new();
    let mut image_files: Vec<PathBuf> = Vec::new();
    let mut video_files: Vec<PathBuf> = Vec::new();
    let mut archive_files: Vec<PathBuf> = Vec::new();

    for f in &files {
        if is_image(f) {
            image_files.push(f.clone());
        } else if is_video(f) {
            video_files.push(f.clone());
        } else if is_archive(f) {
            archive_files.push(f.clone());
        }
        regular_files.push(f.clone());
    }

    // SHA-256 중복 탐지 — \r(flush) 후 단계 시작 메시지
    let _ = log_tx.send("\r".to_string());
    let _ = log_tx.send(format!("해시 중복 탐지 중... ({} 파일)", regular_files.len()));
    let mut regular = {
        let files = regular_files.clone();
        let tx = log_tx.clone();
        let c = cancel.clone();
        let min_size_kb = options.min_size_kb;
        let check_inode = options.check_inode;
        let partial_hash_kb = options.partial_hash_kb;
        let cache_ref = cache.clone();
        tokio::task::spawn_blocking(move || hash::find_duplicates_opts(&files, Some(&tx), Some(&c), min_size_kb, check_inode, partial_hash_kb, cache_ref)).await?
    };
    regular.retain(|g| !is_same_zip_only_group(g));
    // 단계 완료 flush
    if let Some(c) = &cache {
        if let Ok(mut guard) = c.lock() { let _ = guard.flush(); }
    }
    let _ = log_tx.send(format!("해시 중복 그룹: {}개", regular.len()));

    if cancel.is_cancelled() {
        return Ok(ScanResult { regular, ..Default::default() });
    }

    // pHash 이미지 유사도
    let image = if !options.no_phash && !image_files.is_empty() {
        let _ = log_tx.send("\r".to_string());
        let _ = log_tx.send(format!("이미지 유사도 분석 중... ({} 파일)", image_files.len()));
        let exact = options.phash_exact;
        let similar = options.phash_similar;
        let tx = log_tx.clone();
        let c = cancel.clone();
        let cache_ref = cache.clone();
        let result = tokio::task::spawn_blocking(move || {
            phash::find_similar_images_cached(&image_files, exact, similar, Some(&tx), Some(&c), cache_ref)
        }).await?;
        // 단계 완료 flush
        if let Some(c) = &cache {
            if let Ok(mut guard) = c.lock() { let _ = guard.flush(); }
        }
        let _ = log_tx.send(format!("이미지 유사 그룹: {}개", result.len()));
        result
    } else {
        Vec::new()
    };

    if cancel.is_cancelled() {
        return Ok(ScanResult { regular, image, ..Default::default() });
    }

    // vHash 영상 유사도
    let video = if !options.no_vhash && !video_files.is_empty() {
        let _ = log_tx.send("\r".to_string());
        let _ = log_tx.send(format!("영상 유사도 분석 중... ({} 파일)", video_files.len()));
        let n_frames = options.vhash_frames;
        let exact = options.vhash_exact;
        let similar = options.vhash_similar;
        let tx = log_tx.clone();
        let c = cancel.clone();
        let cache_ref = cache.clone();
        let result = tokio::task::spawn_blocking(move || {
            vhash::find_similar_videos_cached(&video_files, n_frames, exact, similar, Some(&tx), Some(&c), cache_ref)
        }).await?;
        // 단계 완료 flush
        if let Some(c) = &cache {
            if let Ok(mut guard) = c.lock() { let _ = guard.flush(); }
        }
        let _ = log_tx.send(format!("영상 유사 그룹: {}개", result.len()));
        result
    } else {
        Vec::new()
    };

    // 아카이브 내부 중복
    let archive = if !options.no_archive && !archive_files.is_empty() {
        let _ = log_tx.send("\r".to_string());
        let _ = log_tx.send(format!("아카이브 중복 분석 중... ({} 파일)", archive_files.len()));
        let min_overlap = options.min_overlap;
        let log_tx2 = log_tx.clone();
        let result = tokio::task::spawn_blocking(move || {
            archive::find_archive_duplicates(&archive_files, min_overlap, Some(&log_tx2))
        }).await?;
        let _ = log_tx.send(format!("아카이브 중복 그룹: {}개", result.len()));
        result
    } else {
        Vec::new()
    };

    let _ = log_tx.send("스캔 완료".to_string());

    Ok(ScanResult { regular, image, video, archive })
}
