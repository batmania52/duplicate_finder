pub mod model;
pub mod collector;
pub mod filter;
pub mod hash;
pub mod phash;
pub mod vhash;
pub mod archive;

use std::path::PathBuf;
use tokio_util::sync::CancellationToken;
use crate::collector::{collect_files, is_image, is_video, is_archive};
use crate::model::{ScanOptions, ScanResult};

pub type LogSender = tokio::sync::mpsc::UnboundedSender<String>;

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
        tokio::task::spawn_blocking(move || hash::find_duplicates_opts(&files, Some(&tx), Some(&c), min_size_kb, check_inode, partial_hash_kb)).await?
    };
    regular.retain(|g| !is_same_zip_only_group(g));
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
        let result = tokio::task::spawn_blocking(move || {
            phash::find_similar_images(&image_files, exact, similar, Some(&tx), Some(&c))
        }).await?;
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
        let result = tokio::task::spawn_blocking(move || {
            vhash::find_similar_videos(&video_files, n_frames, exact, similar, Some(&tx), Some(&c))
        }).await?;
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
