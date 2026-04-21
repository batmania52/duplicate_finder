use std::path::Path;
use std::sync::{Arc, Mutex};
use ffmpeg_next as ffmpeg;
use crate::cache::{CacheEntry, HashCache};
use crate::model::{FileEntry, Group, fmt_size};
use crate::phash::{compute_phash_from_pixels, hamming_distance};
use rayon::prelude::*;

pub fn extract_frame_hashes(path: &Path, n_frames: u32) -> anyhow::Result<Vec<u64>> {
    ffmpeg::init()?;

    let mut ictx = ffmpeg::format::input(path)?;
    let video_stream = ictx
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or(anyhow::anyhow!("영상 스트림 없음"))?;
    let stream_index = video_stream.index();
    let time_base = video_stream.time_base();

    let codec_ctx = ffmpeg::codec::context::Context::from_parameters(video_stream.parameters())?;
    let mut decoder = codec_ctx.decoder().video()?;

    // 영상 길이(초) 계산 — ictx.duration()은 AV_TIME_BASE(1_000_000) 단위
    let duration_sec = if ictx.duration() > 0 {
        ictx.duration() as f64 / 1_000_000.0
    } else {
        0.0
    };

    // 동적 샘플 수: 30분+ 영상은 최소 20프레임
    let actual_n_frames = if duration_sec >= 1800.0 {
        n_frames.max(20)
    } else {
        n_frames
    };

    // seek 방식 사용 여부: 5분 이상이면 seek
    let use_seek = duration_sec >= 300.0;

    let mut scaler = ffmpeg::software::scaling::context::Context::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        ffmpeg::format::Pixel::RGB24,
        32,
        32,
        ffmpeg::software::scaling::flag::Flags::BILINEAR,
    )?;

    if use_seek {
        extract_with_seek(
            &mut ictx, &mut decoder, &mut scaler,
            stream_index, time_base, duration_sec, actual_n_frames,
        ).or_else(|_| {
            // seek 실패 시 순차 디코딩으로 폴백
            let mut ictx2 = ffmpeg::format::input(path)?;
            let video_stream2 = ictx2
                .streams()
                .best(ffmpeg::media::Type::Video)
                .ok_or(anyhow::anyhow!("영상 스트림 없음"))?;
            let stream_index2 = video_stream2.index();
            let codec_ctx2 = ffmpeg::codec::context::Context::from_parameters(video_stream2.parameters())?;
            let mut decoder2 = codec_ctx2.decoder().video()?;
            let mut scaler2 = ffmpeg::software::scaling::context::Context::get(
                decoder2.format(), decoder2.width(), decoder2.height(),
                ffmpeg::format::Pixel::RGB24, 32, 32,
                ffmpeg::software::scaling::flag::Flags::BILINEAR,
            )?;
            extract_sequential(&mut ictx2, &mut decoder2, &mut scaler2, stream_index2, actual_n_frames)
        })
    } else {
        extract_sequential(&mut ictx, &mut decoder, &mut scaler, stream_index, actual_n_frames)
    }
}

/// seek 기반 샘플링: 각 목표 타임스탬프로 직접 seek → 근접 프레임 추출
fn extract_with_seek(
    ictx: &mut ffmpeg::format::context::Input,
    decoder: &mut ffmpeg::decoder::Video,
    scaler: &mut ffmpeg::software::scaling::context::Context,
    stream_index: usize,
    time_base: ffmpeg::Rational,
    duration_sec: f64,
    n_frames: u32,
) -> anyhow::Result<Vec<u64>> {
    let margin = duration_sec * 0.05;
    let start_sec = margin;
    let end_sec = (duration_sec - margin).max(start_sec + 1.0);

    // AV_TIME_BASE (1_000_000) 기준 타임스탬프
    let av_time_base = 1_000_000i64;
    let sample_timestamps: Vec<i64> = (0..n_frames as u64)
        .map(|i| {
            let t = start_sec + (end_sec - start_sec) * i as f64 / (n_frames as f64).max(1.0);
            (t * av_time_base as f64) as i64
        })
        .collect();

    // time_base → AV_TIME_BASE 변환 계수
    let tb_to_av = av_time_base as f64 * time_base.0 as f64 / time_base.1 as f64;

    let mut hashes: Vec<u64> = Vec::new();

    for &target_ts_av in &sample_timestamps {
        if ictx.seek(target_ts_av, ..target_ts_av).is_err() {
            continue;
        }
        decoder.flush();

        // seek 후 최대 60패킷 디코딩해 목표 시간에 가장 가까운 프레임 선택
        let mut best_frame: Option<ffmpeg::frame::Video> = None;
        let mut best_dist = i64::MAX;
        let mut decoded = ffmpeg::frame::Video::empty();
        let mut packet_count = 0;

        'packet_loop: for (stream, packet) in ictx.packets() {
            if stream.index() != stream_index {
                continue;
            }
            if decoder.send_packet(&packet).is_err() {
                break;
            }
            while decoder.receive_frame(&mut decoded).is_ok() {
                let pts_av = (decoded.pts().unwrap_or(0) as f64 * tb_to_av) as i64;
                let dist = (pts_av - target_ts_av).abs();
                if dist < best_dist {
                    best_dist = dist;
                    best_frame = Some(decoded.clone());
                }
                // 목표 시간을 충분히 지났으면 중단
                if pts_av > target_ts_av + av_time_base {
                    break 'packet_loop;
                }
            }
            packet_count += 1;
            if packet_count >= 60 {
                break;
            }
        }

        if let Some(frame) = best_frame {
            let mut rgb_frame = ffmpeg::frame::Video::empty();
            if scaler.run(&frame, &mut rgb_frame).is_ok() {
                let data = rgb_frame.data(0);
                if data.len() >= 32 * 32 * 3 {
                    let pixels: Vec<u8> = data[..32 * 32 * 3]
                        .chunks(3)
                        .map(|c| ((c[0] as u32 + c[1] as u32 + c[2] as u32) / 3) as u8)
                        .collect();
                    if let Some(hash) = compute_phash_from_pixels(&pixels, 32) {
                        hashes.push(hash);
                    }
                }
            }
        }
    }

    Ok(hashes)
}

/// 순차 디코딩 방식 (< 5분 영상 또는 seek 폴백)
fn extract_sequential(
    ictx: &mut ffmpeg::format::context::Input,
    decoder: &mut ffmpeg::decoder::Video,
    scaler: &mut ffmpeg::software::scaling::context::Context,
    stream_index: usize,
    n_frames: u32,
) -> anyhow::Result<Vec<u64>> {
    let duration_raw = ictx.duration();

    let total_frames = if duration_raw > 0 {
        let fps = decoder.frame_rate().unwrap_or(ffmpeg::Rational(25, 1));
        let duration_sec = duration_raw as f64 / 1_000_000.0;
        (duration_sec * fps.0 as f64 / fps.1 as f64) as u64
    } else {
        1000
    };

    let margin = (total_frames as f64 * 0.1) as u64;
    let start = margin;
    let end = total_frames.saturating_sub(margin);
    let sample_count = n_frames as u64;
    let sample_positions: Vec<u64> = (0..sample_count)
        .map(|i| start + i * (end - start).max(1) / sample_count.max(1))
        .collect();

    let mut frame_hashes: Vec<u64> = Vec::new();
    let mut frame_idx: u64 = 0;
    let mut sample_pos_idx: usize = 0;

    let mut decoded = ffmpeg::frame::Video::empty();

    macro_rules! drain_decoder {
        () => {{
            let mut done = false;
            while decoder.receive_frame(&mut decoded).is_ok() {
                if sample_pos_idx >= sample_positions.len() { done = true; break; }
                let target = sample_positions[sample_pos_idx];
                if frame_idx >= target {
                    let mut rgb_frame = ffmpeg::frame::Video::empty();
                    if scaler.run(&decoded, &mut rgb_frame).is_ok() {
                        let data = rgb_frame.data(0);
                        if data.len() >= 32 * 32 * 3 {
                            let pixels: Vec<u8> = data[..32 * 32 * 3]
                                .chunks(3)
                                .map(|c| ((c[0] as u32 + c[1] as u32 + c[2] as u32) / 3) as u8)
                                .collect();
                            if let Some(hash) = compute_phash_from_pixels(&pixels, 32) {
                                frame_hashes.push(hash);
                            }
                        }
                    }
                    sample_pos_idx += 1;
                }
                frame_idx += 1;
            }
            done
        }};
    }

    'outer: for (stream, packet) in ictx.packets() {
        if stream.index() == stream_index {
            if decoder.send_packet(&packet).is_ok() && drain_decoder!() {
                break 'outer;
            }
        }
    }
    decoder.send_eof()?;
    drain_decoder!();

    Ok(frame_hashes)
}

pub fn compare_frame_hashes(hashes_a: &[u64], hashes_b: &[u64]) -> f32 {
    if hashes_a.is_empty() || hashes_b.is_empty() {
        return f32::MAX;
    }
    let total: f32 = hashes_a.iter().map(|&ha| {
        hashes_b.iter().map(|&hb| hamming_distance(ha, hb)).min().unwrap_or(64) as f32
    }).sum();
    total / hashes_a.len() as f32
}

pub fn find_similar_videos(
    files: &[std::path::PathBuf],
    n_frames: u32,
    exact_threshold: f32,
    similar_threshold: f32,
    log_tx: Option<&crate::LogSender>,
    cancel: Option<&tokio_util::sync::CancellationToken>,
) -> Vec<Group> {
    find_similar_videos_cached(files, n_frames, exact_threshold, similar_threshold, log_tx, cancel, None)
}

pub fn find_similar_videos_cached(
    files: &[std::path::PathBuf],
    n_frames: u32,
    exact_threshold: f32,
    similar_threshold: f32,
    log_tx: Option<&crate::LogSender>,
    cancel: Option<&tokio_util::sync::CancellationToken>,
    cache: Option<Arc<Mutex<HashCache>>>,
) -> Vec<Group> {
    if files.is_empty() {
        return Vec::new();
    }

    let total = files.len();
    let done = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let hashed: Vec<(Vec<u64>, &std::path::PathBuf)> = files
        .par_iter()
        .filter_map(|p| {
            if cancel.map(|c| c.is_cancelled()).unwrap_or(false) { return None; }

            let cache_key = HashCache::cache_key(p);

            // 캐시 hit 확인
            if let (Some(key), Some(c)) = (&cache_key, &cache) {
                if let Ok(guard) = c.lock() {
                    if let Some(entry) = guard.get(key) {
                        if let Some(frames) = &entry.vhash_frames {
                            if !frames.is_empty() {
                                let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                                if let Some(tx) = log_tx {
                                    if n % 10 == 0 || n == total {
                                        let _ = tx.send(format!("\r영상 유사도 분석 중... ({} / {})", n, total));
                                    }
                                }
                                return Some((frames.clone(), p));
                            }
                        }
                    }
                }
            }

            let result = extract_frame_hashes(p, n_frames).ok().map(|frames| {
                // 캐시에 저장
                if let (Some(key), Some(c)) = (&cache_key, &cache) {
                    if let Ok(mut guard) = c.lock() {
                        let entry = CacheEntry {
                            vhash_frames: Some(frames.clone()),
                            ..Default::default()
                        };
                        guard.insert(key.clone(), entry);
                        let _ = guard.flush_if_needed();
                    }
                }
                (frames, p)
            });
            let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            if let Some(tx) = log_tx {
                if n % 10 == 0 || n == total {
                    let _ = tx.send(format!("\r영상 유사도 분석 중... ({} / {})", n, total));
                }
            }
            result
        })
        .filter(|(h, _)| !h.is_empty())
        .collect();

    if hashed.len() < 2 {
        return Vec::new();
    }

    let n = hashed.len();
    let mut parent: Vec<usize> = (0..n).collect();
    let mut group_category: std::collections::HashMap<usize, String> = std::collections::HashMap::new();

    fn find(parent: &mut Vec<usize>, x: usize) -> usize {
        if parent[x] != x { parent[x] = find(parent, parent[x]); }
        parent[x]
    }

    for i in 0..n {
        for j in (i + 1)..n {
            let dist = compare_frame_hashes(&hashed[i].0, &hashed[j].0)
                .min(compare_frame_hashes(&hashed[j].0, &hashed[i].0));
            if dist <= similar_threshold {
                let pi = find(&mut parent, i);
                let pj = find(&mut parent, j);
                if pi != pj { parent[pi] = pj; }
                let root = find(&mut parent, i);
                let cat = if dist <= exact_threshold { "exact" } else { "similar" };
                let entry = group_category.entry(root).or_insert_with(|| cat.to_string());
                if cat == "similar" { *entry = "similar".to_string(); }
            }
        }
    }

    let mut by_root: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    for i in 0..n {
        let root = find(&mut parent, i);
        by_root.entry(root).or_default().push(i);
    }

    let mut groups: Vec<Group> = by_root
        .into_iter()
        .filter(|(_, ids)| ids.len() > 1)
        .enumerate()
        .map(|(gi, (root, ids))| {
            let category = group_category.get(&root).cloned().unwrap_or_else(|| "exact".to_string());
            let files_out: Vec<FileEntry> = ids.iter().enumerate().map(|(fi, &id)| {
                let path = hashed[id].1;
                let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                FileEntry {
                    path: path.to_string_lossy().to_string(),
                    size,
                    size_fmt: fmt_size(size),
                    keep: fi == 0,
                    file_type: "file".to_string(),
                    frame_count: Some(hashed[id].0.len().to_string()),
                    ..Default::default()
                }
            }).collect();
            let savable: u64 = files_out.iter().skip(1).map(|f| f.size).sum();
            Group {
                id: format!("v{}", gi + 1),
                category: Some(category),
                savable,
                savable_fmt: fmt_size(savable),
                files: files_out,
                ..Default::default()
            }
        })
        .collect();

    groups.sort_by(|a, b| b.savable.cmp(&a.savable));
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_frame_hashes_identical() {
        let hashes = vec![0u64, 1u64, 2u64];
        assert_eq!(compare_frame_hashes(&hashes, &hashes), 0.0);
    }

    #[test]
    fn test_compare_frame_hashes_different() {
        let a = vec![0u64];
        let b = vec![u64::MAX];
        assert_eq!(compare_frame_hashes(&a, &b), 64.0);
    }
}
