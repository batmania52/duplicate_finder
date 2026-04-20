use std::collections::HashMap;
use std::path::Path;
use image::imageops::FilterType;
use rayon::prelude::*;
use crate::model::{FileEntry, Group, fmt_size};

const HASH_SIZE: u32 = 8; // 8x8 DCT → 64bit hash

pub fn compute_phash(path: &Path) -> Option<u64> {
    let img = image::open(path).ok()?;
    let gray = img.resize_exact(HASH_SIZE + 1, HASH_SIZE + 1, FilterType::Lanczos3)
        .grayscale();
    let pixels: Vec<f32> = gray.to_luma8().pixels().map(|p| p.0[0] as f32).collect();

    // 평균 기반 pHash (DCT 근사)
    let w = (HASH_SIZE + 1) as usize;
    let mut dct = vec![0f32; (HASH_SIZE * HASH_SIZE) as usize];
    for y in 0..HASH_SIZE as usize {
        for x in 0..HASH_SIZE as usize {
            // 수평 차분 (간단 DCT 근사)
            dct[y * HASH_SIZE as usize + x] = pixels[y * w + x + 1] - pixels[y * w + x];
        }
    }

    let mean = dct.iter().sum::<f32>() / dct.len() as f32;
    let hash = dct.iter().enumerate().fold(0u64, |acc, (i, &v)| {
        if v > mean { acc | (1u64 << i) } else { acc }
    });
    Some(hash)
}

/// 이미 디코딩된 grayscale 픽셀 배열로 pHash 계산 (vhash에서 사용)
pub fn compute_phash_from_pixels(pixels: &[u8], size: u32) -> Option<u64> {
    if pixels.len() < (size * size) as usize {
        return None;
    }
    let w = size as usize;
    let hash_size = HASH_SIZE as usize;
    // size→(HASH_SIZE+1) 다운샘플: 간단 박스 필터
    let scale = w as f32 / (hash_size + 1) as f32;
    let mut resized = vec![0f32; (hash_size + 1) * (hash_size + 1)];
    for dy in 0..=(hash_size) {
        for dx in 0..=(hash_size) {
            let sy = (dy as f32 * scale) as usize;
            let sx = (dx as f32 * scale) as usize;
            let sy = sy.min(w - 1);
            let sx = sx.min(w - 1);
            resized[dy * (hash_size + 1) + dx] = pixels[sy * w + sx] as f32;
        }
    }
    let mut dct = vec![0f32; hash_size * hash_size];
    for y in 0..hash_size {
        for x in 0..hash_size {
            dct[y * hash_size + x] = resized[y * (hash_size + 1) + x + 1] - resized[y * (hash_size + 1) + x];
        }
    }
    let mean = dct.iter().sum::<f32>() / dct.len() as f32;
    let hash = dct.iter().enumerate().fold(0u64, |acc, (i, &v)| {
        if v > mean { acc | (1u64 << i) } else { acc }
    });
    Some(hash)
}

pub fn hamming_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

// BK-tree 구현
pub struct BkTree {
    nodes: Vec<BkNode>,
}

struct BkNode {
    hash: u64,
    id: usize,
    children: HashMap<u32, usize>,
}

impl BkTree {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn insert(&mut self, hash: u64, id: usize) {
        if self.nodes.is_empty() {
            self.nodes.push(BkNode { hash, id, children: HashMap::new() });
            return;
        }
        let mut cur = 0;
        loop {
            let dist = hamming_distance(hash, self.nodes[cur].hash);
            if dist == 0 {
                return; // 완전 동일 해시는 중복 삽입 생략
            }
            if let Some(&child_idx) = self.nodes[cur].children.get(&dist) {
                cur = child_idx;
            } else {
                let new_idx = self.nodes.len();
                self.nodes[cur].children.insert(dist, new_idx);
                self.nodes.push(BkNode { hash, id, children: HashMap::new() });
                return;
            }
        }
    }

    pub fn find(&self, query: u64, max_dist: u32) -> Vec<(usize, u32)> {
        if self.nodes.is_empty() {
            return Vec::new();
        }
        let mut result = Vec::new();
        let mut stack = vec![0usize];
        while let Some(cur) = stack.pop() {
            let node = &self.nodes[cur];
            let dist = hamming_distance(query, node.hash);
            if dist <= max_dist {
                result.push((node.id, dist));
            }
            let lo = dist.saturating_sub(max_dist);
            let hi = dist + max_dist;
            for (&edge_dist, &child_idx) in &node.children {
                if edge_dist >= lo && edge_dist <= hi {
                    stack.push(child_idx);
                }
            }
        }
        result
    }
}

pub fn find_similar_images(
    files: &[std::path::PathBuf],
    exact_threshold: u32,
    similar_threshold: u32,
) -> Vec<Group> {
    // 병렬 pHash 계산
    let hashed: Vec<(u64, &std::path::PathBuf)> = files
        .par_iter()
        .filter_map(|p| compute_phash(p).map(|h| (h, p)))
        .collect();

    if hashed.is_empty() {
        return Vec::new();
    }

    // BK-tree 구축
    let mut tree = BkTree::new();
    for (i, (hash, _)) in hashed.iter().enumerate() {
        tree.insert(*hash, i);
    }

    // 그룹화 (Union-Find)
    let n = hashed.len();
    let mut parent: Vec<usize> = (0..n).collect();
    let mut edge_dist: Vec<u32> = vec![0; n]; // 그룹 내 최대 거리 추적

    fn find(parent: &mut Vec<usize>, x: usize) -> usize {
        if parent[x] != x { parent[x] = find(parent, parent[x]); }
        parent[x]
    }

    let mut group_category: HashMap<usize, String> = HashMap::new();

    for (i, (hash, _)) in hashed.iter().enumerate() {
        let matches = tree.find(*hash, similar_threshold);
        for (j, dist) in matches {
            if j == i { continue; }
            let pi = find(&mut parent, i);
            let pj = find(&mut parent, j);
            if pi != pj {
                parent[pi] = pj;
            }
            let root = find(&mut parent, i);
            let prev = edge_dist[root];
            edge_dist[root] = prev.max(dist);
            let cat = if dist <= exact_threshold { "exact" } else { "similar" };
            let entry = group_category.entry(root).or_insert_with(|| cat.to_string());
            if cat == "similar" { *entry = "similar".to_string(); }
        }
    }

    // 루트별 파일 모으기
    let mut by_root: HashMap<usize, Vec<usize>> = HashMap::new();
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
            let files: Vec<FileEntry> = ids.iter().enumerate().map(|(fi, &id)| {
                let (hash, path) = &hashed[id];
                let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                FileEntry {
                    path: path.to_string_lossy().to_string(),
                    size,
                    size_fmt: fmt_size(size),
                    keep: fi == 0,
                    phash: Some(format!("{:016x}", hash)),
                    file_type: "file".to_string(),
                    ..Default::default()
                }
            }).collect();
            let savable: u64 = files.iter().skip(1).map(|f| f.size).sum();
            Group {
                id: format!("i{}", gi + 1),
                category: Some(category),
                savable,
                savable_fmt: fmt_size(savable),
                files,
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
    fn test_hamming_distance() {
        assert_eq!(hamming_distance(0b1010, 0b1010), 0);
        assert_eq!(hamming_distance(0b1010, 0b0101), 4);
        assert_eq!(hamming_distance(0u64, u64::MAX), 64);
    }

    #[test]
    fn test_bktree_insert_find() {
        let mut tree = BkTree::new();
        tree.insert(0b0000_0000u64, 0);
        tree.insert(0b0000_0001u64, 1); // distance 1
        tree.insert(0b1111_1111u64, 2); // distance 8

        let results = tree.find(0b0000_0000u64, 2);
        let ids: Vec<usize> = results.iter().map(|(id, _)| *id).collect();
        assert!(ids.contains(&0));
        assert!(ids.contains(&1));
        assert!(!ids.contains(&2));
    }
}
