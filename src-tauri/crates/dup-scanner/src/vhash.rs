use std::path::Path;
use ffmpeg_next as ffmpeg;
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

    let codec_ctx = ffmpeg::codec::context::Context::from_parameters(video_stream.parameters())?;
    let mut decoder = codec_ctx.decoder().video()?;

    let duration = ictx.duration();
    let time_base = video_stream.time_base();
    let total_frames = if duration > 0 && time_base.1 > 0 {
        let fps = decoder.frame_rate().unwrap_or(ffmpeg::Rational(25, 1));
        (duration as f64 * time_base.0 as f64 / time_base.1 as f64
            * fps.0 as f64 / fps.1 as f64) as u64
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

    let mut scaler = ffmpeg::software::scaling::context::Context::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        ffmpeg::format::Pixel::RGB24,
        32,
        32,
        ffmpeg::software::scaling::flag::Flags::BILINEAR,
    )?;

    let mut frame_hashes: Vec<u64> = Vec::new();
    let mut frame_idx: u64 = 0;
    let mut sample_pos_idx: usize = 0;

    let process_decoded = |decoder: &mut ffmpeg::decoder::Video,
                                scaler: &mut ffmpeg::software::scaling::context::Context,
                                frame_idx: &mut u64,
                                sample_pos_idx: &mut usize,
                                hashes: &mut Vec<u64>| -> anyhow::Result<bool> {
        let mut decoded = ffmpeg::frame::Video::empty();
        while decoder.receive_frame(&mut decoded).is_ok() {
            if *sample_pos_idx >= sample_positions.len() {
                return Ok(true); // done
            }
            let target = sample_positions[*sample_pos_idx];
            if *frame_idx >= target {
                let mut rgb_frame = ffmpeg::frame::Video::empty();
                scaler.run(&decoded, &mut rgb_frame)?;
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
                *sample_pos_idx += 1;
            }
            *frame_idx += 1;
        }
        Ok(false)
    };

    for (stream, packet) in ictx.packets() {
        if stream.index() == stream_index {
            decoder.send_packet(&packet)?;
            let done = process_decoded(
                &mut decoder,
                &mut scaler,
                &mut frame_idx,
                &mut sample_pos_idx,
                &mut frame_hashes,
            )?;
            if done {
                break;
            }
        }
    }
    decoder.send_eof()?;
    process_decoded(
        &mut decoder,
        &mut scaler,
        &mut frame_idx,
        &mut sample_pos_idx,
        &mut frame_hashes,
    )?;

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
    if files.is_empty() {
        return Vec::new();
    }

    let total = files.len();
    let done = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let hashed: Vec<(Vec<u64>, &std::path::PathBuf)> = files
        .par_iter()
        .filter_map(|p| {
            if cancel.map(|c| c.is_cancelled()).unwrap_or(false) { return None; }
            let result = extract_frame_hashes(p, n_frames).ok().map(|h| (h, p));
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
            let dist = compare_frame_hashes(&hashed[i].0, &hashed[j].0);
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
