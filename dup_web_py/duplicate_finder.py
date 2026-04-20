#!/usr/bin/env python3
"""
duplicate_finder.py
NAS 중복 파일 검사 도구 (압축파일 내부 + 이미지 유사도 포함)

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
사용법
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  # 기본 검사 (단일 디렉토리)
  python duplicate_finder.py scan /Volumes/NAS/photos

  # 여러 디렉토리 동시 검사
  python duplicate_finder.py scan /Volumes/NAS/photos /Volumes/NAS/backup

  # 임계값 조정
  python duplicate_finder.py scan /Volumes/NAS/ --phash-exact 3 --phash-similar 15

  # 이미지 유사도 검사 끄기
  python duplicate_finder.py scan /Volumes/NAS/ --no-phash

  # 압축파일 내부 검사 끄기
  python duplicate_finder.py scan /Volumes/NAS/ --no-archive

  # 압축파일 간 겹침 탐지 기준 변경 (기본 2개)
  python duplicate_finder.py scan /Volumes/NAS/ --min-overlap 5

  # 결과 CSV 파일명 직접 지정
  python duplicate_finder.py scan /Volumes/NAS/ -o my_result.csv

  # 중복 삭제 (dry-run: 실제 삭제 없이 목록만 확인)
  python duplicate_finder.py delete duplicates_20240101_120000.csv

  # 중복 삭제 (실제 삭제 실행)
  python duplicate_finder.py delete duplicates_20240101_120000.csv --no-dry-run

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
출력 CSV 종류
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  duplicates_날짜.csv        해시 기반 완전 동일 파일 목록
  image_similar_날짜.csv     유사 이미지 목록 (exact / similar 그룹 분리)
  archive_overlaps_날짜.csv  n개 이상 내용물이 겹치는 압축파일 쌍 목록
  video_similar_날짜.csv     유사 영상 목록 (exact / similar 그룹 분리)

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
pHash 해밍 거리 임계값 설명
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  pHash는 이미지를 64비트 해시로 변환하고,
  두 해시의 다른 비트 수(해밍 거리)로 유사도를 측정해요.
  거리 0 = 완전 동일 / 거리 64 = 완전 다름

  --phash-exact  N   거리 ≤ N 이면 "완전동일" 그룹 (category=exact)
                     해상도/메타데이터만 다른 경우를 잡음
                     기본값: 0 (픽셀 완전 동일만)

  --phash-similar N  거리 ≤ N 이면 "유사" 그룹 (category=similar)
                     리사이즈, 밝기/대비 보정 등을 잡음
                     exact 그룹과 중복되지 않게 별도 표시
                     기본값: 10

  권장 조합:
    보수적 (오탐 최소화)  --phash-exact 0  --phash-similar 8
    일반적               --phash-exact 3  --phash-similar 15
    공격적 (수동 확인 필요) --phash-exact 5  --phash-similar 20

  주의: similar 값을 너무 높이면 다른 사진이 같은 그룹으로
        묶일 수 있으므로 결과를 직접 확인하는 것을 권장해요.

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
의존 라이브러리 설치
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  pip install imagehash Pillow
  brew install ffmpeg   # macOS
  # apt install ffmpeg  # Linux

  (미설치 시 해당 검사만 자동으로 비활성화됩니다)

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
영상 pHash 옵션 설명
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  --vhash-frames N    영상당 샘플링 프레임 수 (기본: 10)
                      앞뒤 10% 제외 후 균등 간격으로 추출
                      많을수록 정확하지만 느림
                        8~10  : 일반적 중복 탐지 (권장)
                        16~32 : 편집본/클립 비교 시
                        3~5   : 빠른 초벌 검사

  --vhash-exact N     프레임 평균 해밍 거리 ≤ N → 완전동일 (기본: 3.0)
                      재인코딩, 컨테이너 변환(mp4↔mkv) 등을 잡음

  --vhash-similar N   프레임 평균 해밍 거리 ≤ N → 유사 (기본: 10.0)
                      밝기 조정, 약간의 크롭/리사이즈 등을 잡음

  --no-vhash          영상 유사도 검사 비활성화

  주의: 영상 수가 많으면 매우 오래 걸려요.
        --no-vhash 로 끄고 이미지/파일 중복만 먼저 처리하는 것을 권장해요.
"""

import io
import os
import sys
import shutil
import hashlib
import zipfile
import tarfile
import csv
import json
import argparse
import subprocess
import threading
import fnmatch
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime
from collections import defaultdict
from pathlib import Path

# pHash 관련 (없으면 이미지 유사도 검사 비활성화)
try:
    import imagehash
    from PIL import Image
    PHASH_AVAILABLE = True
except ImportError:
    PHASH_AVAILABLE = False

# BK-tree (cppbktree 우선, pybktree 폴백, 없으면 O(n²))
try:
    import cppbktree
    CPPBKTREE_AVAILABLE = True
    PYBKTREE_AVAILABLE = False
except ImportError:
    CPPBKTREE_AVAILABLE = False
    try:
        import pybktree
        PYBKTREE_AVAILABLE = True
    except ImportError:
        PYBKTREE_AVAILABLE = False

# ffmpeg 가용 여부 체크
FFMPEG_AVAILABLE = shutil.which('ffmpeg') is not None

_VDIST_SCALE = 10  # 영상 거리(float)를 BK-tree 정수 키로 변환할 배율

# ──────────────────────────────────────────────
# 설정
# ──────────────────────────────────────────────
CHUNK_SIZE = 65536          # 해시 계산 청크 크기 (64KB)
PARTIAL_HASH_SIZE = 65536   # 1차 필터용 부분 해시 크기 (64KB)
PROGRESS_INTERVAL = 100     # N개 파일마다 진행상황 출력
_WORKERS = min(os.cpu_count() or 4, 16)  # ThreadPoolExecutor worker 수

# 이미지 확장자
IMAGE_EXTENSIONS = {'.jpg', '.jpeg', '.png', '.gif', '.bmp', '.webp', '.tiff', '.tif', '.heic', '.heif'}

# 영상 확장자
VIDEO_EXTENSIONS = {'.mp4', '.mkv', '.avi', '.mov', '.wmv', '.flv', '.webm', '.m4v', '.mpg', '.mpeg', '.ts', '.mts', '.m2ts'}

# 무시할 파일/디렉토리 패턴
IGNORE_NAMES = {'.DS_Store', '.Spotlight-V100', '.Trashes', '.fseventsd', 'Thumbs.db', 'desktop.ini'}
IGNORE_EXTENSIONS = {'.tmp', '.temp', '.part'}


# ──────────────────────────────────────────────
# 해시 계산
# ──────────────────────────────────────────────
def hash_file(filepath: str, partial: bool = False) -> str | None:
    """파일 해시 계산 (partial=True면 앞 64KB만)"""
    try:
        h = hashlib.sha256()
        with open(filepath, 'rb') as f:
            if partial:
                data = f.read(PARTIAL_HASH_SIZE)
                h.update(data)
            else:
                while chunk := f.read(CHUNK_SIZE):
                    h.update(chunk)
        return h.hexdigest()
    except (PermissionError, OSError):
        return None


def hash_bytes(data: bytes) -> str:
    """바이트 데이터 해시 계산 (압축파일 내부용)"""
    return hashlib.sha256(data).hexdigest()



# ──────────────────────────────────────────────
# 이미지 pHash (유사 이미지 탐지)
# ──────────────────────────────────────────────
def is_image(path: str) -> bool:
    return Path(path).suffix.lower() in IMAGE_EXTENSIONS


def compute_phash(filepath: str) -> str | None:
    """파일 경로로 pHash 계산"""
    if not PHASH_AVAILABLE:
        return None
    try:
        with Image.open(filepath) as img:
            return str(imagehash.phash(img))
    except Exception:
        return None


def compute_phash_from_bytes(data: bytes, filename: str) -> str | None:
    """바이트 데이터로 pHash 계산 (압축 내부 이미지용)"""
    if not PHASH_AVAILABLE:
        return None
    ext = Path(filename).suffix.lower()
    if ext not in IMAGE_EXTENSIONS:
        return None
    try:
        with Image.open(io.BytesIO(data)) as img:
            return str(imagehash.phash(img))
    except Exception:
        return None


def phash_distance(h1: str, h2: str) -> int:
    """두 pHash 간 해밍 거리 계산 (0=완전동일, 64=완전다름)"""
    try:
        return imagehash.hex_to_hash(h1) - imagehash.hex_to_hash(h2)
    except Exception:
        return 64


def make_uf(n: int) -> tuple:
    """Union-Find (경로 압축 포함)"""
    parent = list(range(n))
    def find(x: int) -> int:
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x
    def union(x: int, y: int) -> None:
        parent[find(x)] = find(y)
    return find, union


# ──────────────────────────────────────────────
# BK-tree 어댑터
# ──────────────────────────────────────────────

def _bktree_image_find(
    hash_ints: list[int],
    query_int: int,
    threshold: int,
) -> list[tuple[int, int]]:
    """해밍 거리 threshold 이내 항목 탐색. 반환: [(dist, j), ...] (j = hash_ints index)"""
    if CPPBKTREE_AVAILABLE:
        # cppbktree.BKTree64.find()는 삽입 순서 기준 index를 반환한다
        tree = cppbktree.BKTree64(hash_ints)
        return [(bin(query_int ^ hash_ints[j]).count('1'), j) for j in tree.find(query_int, threshold)]
    elif PYBKTREE_AVAILABLE:
        def _hamming(a: tuple, b: tuple) -> int:
            return bin(a[0] ^ b[0]).count('1')
        items_with_idx = [(h, i) for i, h in enumerate(hash_ints)]
        tree = pybktree.BKTree(_hamming, items_with_idx)
        return [(dist, j) for dist, (_, j) in tree.find((query_int, 0), threshold)]
    else:
        return []


# ──────────────────────────────────────────────
# 병렬화 헬퍼
# ──────────────────────────────────────────────

def _parallel_hash(flat_items: list[dict], partial: bool, progress_cb=None) -> None:
    """flat_items의 각 item에 partial_hash 또는 full_hash를 병렬로 채운다 (in-place)."""
    key = 'partial_hash' if partial else 'full_hash'
    label = '부분' if partial else '전체'
    total = len(flat_items)
    lock = threading.Lock()
    done_count = [0]

    def _compute(item: dict) -> None:
        h = hash_file(item['path'], partial=partial) if item['type'] == 'file' else item.get('hash')
        if h:
            item[key] = h
        with lock:
            done_count[0] += 1
            if done_count[0] % 20 == 0:
                print(f"  {label} 해시 계산 중... {done_count[0]}/{total}", end='\r')
                if progress_cb:
                    progress_cb(done_count[0], total)

    with ThreadPoolExecutor(max_workers=_WORKERS) as pool:
        list(pool.map(_compute, flat_items))


def _parallel_phash(image_items: list[dict], progress_cb=None) -> None:
    """image_items 각 항목에 phash 필드를 병렬로 채운다 (in-place)."""
    total = len(image_items)
    lock = threading.Lock()
    done_count = [0]

    def _compute(it: dict) -> None:
        ph = compute_phash(it['path']) if it['type'] == 'file' else it.get('phash')
        if ph:
            it['phash'] = ph
        with lock:
            done_count[0] += 1
            if done_count[0] % 50 == 0:
                print(f"  {done_count[0]}/{total} 완료", end='\r')
                if progress_cb:
                    progress_cb(done_count[0], total)

    with ThreadPoolExecutor(max_workers=_WORKERS) as pool:
        list(pool.map(_compute, image_items))


def _parallel_vphash(video_items: list[dict], n_frames: int, progress_cb=None) -> None:
    """video_items 각 항목에 video_phashes 필드를 병렬로 채운다 (in-place)."""
    total = len(video_items)
    lock = threading.Lock()
    done_count = [0]

    def _compute(it: dict) -> None:
        hashes = compute_video_phash(it['path'], n_frames=n_frames)
        if hashes:
            it['video_phashes'] = hashes
        with lock:
            done_count[0] += 1
            name = os.path.basename(it['path'])
            print(f"  ({done_count[0]}/{total}) {name}", end='\r')
            if progress_cb:
                progress_cb(done_count[0], total, name)

    with ThreadPoolExecutor(max_workers=min(_WORKERS, 8)) as pool:
        list(pool.map(_compute, video_items))


def find_similar_images(items: list[dict], exact_threshold: int = 0, similar_threshold: int = 10, progress_cb=None) -> tuple[list, list]:
    """
    pHash 기반 이미지 유사도 그룹핑
    - exact_threshold  : 해밍 거리 <= 이 값이면 '완전 동일' 그룹 (해상도/메타데이터만 다름)
    - similar_threshold: 해밍 거리 <= 이 값이면 '유사' 그룹 (리사이즈, 밝기 보정 등)
    반환: (exact_groups, similar_groups)
    """
    image_items = [it for it in items if it.get('phash')]
    if not image_items:
        return [], []

    n = len(image_items)
    print(f"\n[이미지 유사도 분석] {len(image_items)}개 이미지 비교 중...")

    find_e, union_e = make_uf(n)
    find_s, union_s = make_uf(n)

    if CPPBKTREE_AVAILABLE or PYBKTREE_AVAILABLE:
        hash_ints = [int(it['phash'], 16) for it in image_items]
        for i, h in enumerate(hash_ints):
            for dist, j in _bktree_image_find(hash_ints, h, similar_threshold):
                if j <= i:
                    continue
                if dist <= exact_threshold:
                    union_e(i, j)
                union_s(i, j)
            if (i + 1) % 500 == 0:
                print(f"  BK-tree 쿼리 중... {i+1}/{n}", end='\r')
                if progress_cb:
                    progress_cb(i + 1, n)
    else:
        total_pairs = n * (n - 1) // 2
        done = 0
        for i in range(n):
            for j in range(i + 1, n):
                dist = phash_distance(image_items[i]['phash'], image_items[j]['phash'])
                if dist <= exact_threshold:
                    union_e(i, j)
                if dist <= similar_threshold:
                    union_s(i, j)
                done += 1
                if done % 5000 == 0:
                    print(f"  비교 중... {done:,}/{total_pairs:,}", end='\r')
                    if progress_cb:
                        progress_cb(done, total_pairs)

    exact_map = defaultdict(list)
    for i, item in enumerate(image_items):
        exact_map[find_e(i)].append(item)
    exact_groups = [g for g in exact_map.values() if len(g) > 1]

    exact_paths = {it['path'] for g in exact_groups for it in g}
    similar_map = defaultdict(list)
    for i, item in enumerate(image_items):
        similar_map[find_s(i)].append(item)
    similar_groups = [
        g for g in similar_map.values()
        if len(g) > 1 and not any(it['path'] in exact_paths for it in g)
    ]

    print(f"  완전동일 그룹: {len(exact_groups)}개 / 유사 그룹: {len(similar_groups)}개      ")
    return exact_groups, similar_groups


def save_image_csv(exact_groups: list, similar_groups: list, output_path: str):
    """이미지 유사도 결과 CSV 저장"""
    with open(output_path, 'w', newline='', encoding='utf-8') as f:
        writer = csv.writer(f)
        writer.writerow(['category', 'group_id', 'path', 'size_bytes', 'type', 'phash', 'keep'])

        for i, group in enumerate(exact_groups, 1):
            for j, item in enumerate(group):
                writer.writerow([
                    'exact', i, item['path'], item['size'],
                    item.get('type', 'file'), item.get('phash', ''),
                    'YES' if j == 0 else 'NO',
                ])

        for i, group in enumerate(similar_groups, 1):
            for j, item in enumerate(group):
                writer.writerow([
                    'similar', i, item['path'], item['size'],
                    item.get('type', 'file'), item.get('phash', ''),
                    'YES' if j == 0 else 'NO',
                ])
    print(f"[저장 완료] {output_path}")


# ──────────────────────────────────────────────
# 압축파일 내부 파일 목록 추출
# ──────────────────────────────────────────────
def extract_zip_entries(filepath: str) -> list[dict]:
    """zip 내부 파일 목록과 해시 반환"""
    entries = []
    try:
        with zipfile.ZipFile(filepath, 'r') as zf:
            for info in zf.infolist():
                if info.is_dir():
                    continue
                try:
                    data = zf.read(info.filename)
                    entry = {
                        'path': f"{filepath}::{info.filename}",
                        'size': info.file_size,
                        'hash': hash_bytes(data),
                        'type': 'zip_entry',
                    }
                    ph = compute_phash_from_bytes(data, info.filename)
                    if ph:
                        entry['phash'] = ph
                    entries.append(entry)
                except Exception:
                    pass
    except (zipfile.BadZipFile, Exception):
        pass
    return entries


def extract_tar_entries(filepath: str) -> list[dict]:
    """tar/tar.gz 내부 파일 목록과 해시 반환"""
    entries = []
    try:
        with tarfile.open(filepath, 'r:*') as tf:
            for member in tf.getmembers():
                if not member.isfile():
                    continue
                try:
                    f = tf.extractfile(member)
                    if f:
                        data = f.read()
                        entry = {
                            'path': f"{filepath}::{member.name}",
                            'size': member.size,
                            'hash': hash_bytes(data),
                            'type': 'tar_entry',
                        }
                        ph = compute_phash_from_bytes(data, member.name)
                        if ph:
                            entry['phash'] = ph
                        entries.append(entry)
                except Exception:
                    pass
    except Exception:
        pass
    return entries


# ──────────────────────────────────────────────
# 파일 탐색
# ──────────────────────────────────────────────
def collect_files(root: str, progress_cb=None, exclude_patterns: list[str] | None = None) -> list[dict]:
    """디렉토리 재귀 탐색으로 파일 목록 수집"""
    files = []
    _excludes = exclude_patterns or []

    print(f"[탐색 시작] {root}")
    count = 0

    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in IGNORE_NAMES]

        for filename in filenames:
            if filename in IGNORE_NAMES:
                continue
            ext = Path(filename).suffix.lower()
            if ext in IGNORE_EXTENSIONS:
                continue

            filepath = os.path.join(dirpath, filename)
            if _excludes and any(
                fnmatch.fnmatch(filename, p) or fnmatch.fnmatch(filepath, p)
                for p in _excludes
            ):
                continue
            try:
                st = os.stat(filepath)
                size = st.st_size
                created_ts = getattr(st, 'st_birthtime', st.st_ctime)
                created_iso = datetime.fromtimestamp(created_ts).strftime('%Y-%m-%d %H:%M:%S')
                files.append({
                    'path': filepath,
                    'size': size,
                    'hash': None,
                    'type': 'file',
                    'is_image': ext in IMAGE_EXTENSIONS,
                    'is_video': ext in VIDEO_EXTENSIONS,
                    'created': created_iso,
                })
                count += 1
                if count % PROGRESS_INTERVAL == 0:
                    print(f"  탐색 중... {count}개 발견", end='\r')
                    if progress_cb:
                        progress_cb(count, dirpath)
            except OSError:
                pass

    print(f"  탐색 완료: {count}개 파일 발견      ")
    return files


def collect_archive_entries(files: list[dict], progress_cb=None) -> list[dict]:
    """압축파일 내부 항목 추출 (병렬)
    progress_cb(done, total, filename): 진행률 콜백 (선택)
    """
    archives = [f for f in files if Path(f['path']).suffix.lower() in {'.zip', '.gz', '.tgz', '.tar', '.bz2'}]

    if not archives:
        return []

    print(f"\n[압축파일 검사] {len(archives)}개 압축파일 내부 검사 중...")
    lock = threading.Lock()
    done_count = [0]
    total = len(archives)

    def _extract(arc: dict) -> list[dict]:
        path = arc['path']
        ext = Path(path).suffix.lower()
        if ext == '.zip':
            arc_entries = extract_zip_entries(path)
        elif ext in {'.gz', '.tgz', '.tar', '.bz2'}:
            arc_entries = extract_tar_entries(path)
        else:
            arc_entries = []
        for entry in arc_entries:
            entry['arc_total'] = len(arc_entries)
        with lock:
            done_count[0] += 1
            print(f"  ({done_count[0]}/{total}) {os.path.basename(path)}", end='\r')
            if progress_cb:
                progress_cb(done_count[0], total, os.path.basename(path))
        return arc_entries

    entries = []
    with ThreadPoolExecutor(max_workers=min(_WORKERS, 8)) as pool:
        for arc_entries in pool.map(_extract, archives):
            entries.extend(arc_entries)

    print(f"  압축파일 내부 항목: {len(entries)}개 추출 완료      ")
    return entries


# ──────────────────────────────────────────────
# 중복 탐지
# ──────────────────────────────────────────────
def find_duplicates(all_items: list[dict], progress_cb=None) -> list[list[dict]]:
    """크기 → 부분해시 → 전체해시 순으로 중복 탐지"""

    # 1단계: 크기로 후보 그룹화
    size_groups = defaultdict(list)
    for item in all_items:
        size_groups[item['size']].append(item)

    candidates = [g for g in size_groups.values() if len(g) > 1]
    flat_candidates = [item for group in candidates for item in group]
    print(f"\n[해시 계산] 크기 동일 후보: {len(flat_candidates)}개")

    # 2단계: 부분 해시로 재필터 (병렬)
    def _partial_cb(done, total):
        if progress_cb:
            progress_cb('partial', done, total)
    _parallel_hash(flat_candidates, partial=True, progress_cb=_partial_cb)
    print()

    partial_groups = defaultdict(list)
    for item in flat_candidates:
        ph = item.get('partial_hash')
        if ph:
            partial_groups[(item['size'], ph)].append(item)

    candidates2 = [g for g in partial_groups.values() if len(g) > 1]
    flat_candidates2 = [item for group in candidates2 for item in group]
    print(f"  부분 해시 후보: {len(flat_candidates2)}개")

    # 3단계: 전체 해시로 최종 확인 (병렬)
    def _full_cb(done, total):
        if progress_cb:
            progress_cb('full', done, total)
    _parallel_hash(flat_candidates2, partial=False, progress_cb=_full_cb)
    print()

    hash_groups = defaultdict(list)
    for item in flat_candidates2:
        fh = item.get('full_hash')
        if fh:
            hash_groups[fh].append(item)

    duplicates = [g for g in hash_groups.values() if len(g) > 1]
    print(f"  중복 그룹: {len(duplicates)}개 발견      ")
    return duplicates



# ──────────────────────────────────────────────
# 압축파일 간 겹침 분석
# ──────────────────────────────────────────────
def find_archive_overlaps(archive_entries: list[dict], min_overlap: int, min_arc_files: int = 0) -> list[dict]:
    """n개 이상 동일한 내용물을 가진 압축파일 쌍/그룹 탐지.
    min_arc_files: 압축파일 내 파일 수가 이 값 미만이면 비교 대상에서 제외 (0=제한없음)
    """

    # 압축파일별 내부 파일 해시 집합 구성
    # path 형식: "/path/to/archive.zip::internal/file.txt"
    archive_contents: dict[str, set[str]] = defaultdict(set)
    archive_entry_count: dict[str, int] = defaultdict(int)

    for entry in archive_entries:
        # "::" 기준으로 압축파일 경로와 내부 경로 분리
        parts = entry['path'].split('::', 1)
        if len(parts) != 2:
            continue
        arc_path = parts[0]
        fhash = entry.get('hash', '')
        if fhash:
            archive_contents[arc_path].add(fhash)
            archive_entry_count[arc_path] += 1

    # min_arc_files 필터
    if min_arc_files > 0:
        arc_list = [a for a in archive_contents if archive_entry_count[a] >= min_arc_files]
    else:
        arc_list = list(archive_contents.keys())
    overlaps = []

    # 모든 압축파일 쌍 비교
    for i in range(len(arc_list)):
        for j in range(i + 1, len(arc_list)):
            a = arc_list[i]
            b = arc_list[j]
            common = archive_contents[a] & archive_contents[b]
            if len(common) >= min_overlap:
                overlaps.append({
                    'archive_a': a,
                    'archive_b': b,
                    'common_file_count': len(common),
                    'a_total': archive_entry_count[a],
                    'b_total': archive_entry_count[b],
                    'common_hashes': sorted(common),
                })

    # 겹침 수 내림차순 정렬
    overlaps.sort(key=lambda x: x['common_file_count'], reverse=True)
    return overlaps


def save_archive_overlap_csv(overlaps: list[dict], output_path: str):
    """압축파일 겹침 결과 CSV 저장"""
    with open(output_path, 'w', newline='', encoding='utf-8') as f:
        writer = csv.writer(f)
        writer.writerow([
            'archive_a', 'archive_b',
            'common_files', 'a_total', 'b_total',
            'overlap_ratio_a(%)', 'overlap_ratio_b(%)',
        ])
        for ov in overlaps:
            ratio_a = ov['common_file_count'] / ov['a_total'] * 100 if ov['a_total'] else 0
            ratio_b = ov['common_file_count'] / ov['b_total'] * 100 if ov['b_total'] else 0
            writer.writerow([
                ov['archive_a'],
                ov['archive_b'],
                ov['common_file_count'],
                ov['a_total'],
                ov['b_total'],
                f"{ratio_a:.1f}",
                f"{ratio_b:.1f}",
            ])
    print(f"[저장 완료] {output_path}")


# ──────────────────────────────────────────────
# CSV 저장
# ──────────────────────────────────────────────
def _fmt_size(n: int) -> str:
    for unit, threshold in [('TB', 1 << 40), ('GB', 1 << 30), ('MB', 1 << 20), ('KB', 1 << 10)]:
        if n >= threshold:
            return f"{n / threshold:.1f} {unit}"
    return f"{n} B"


def save_csv(duplicates: list[list[dict]], output_path: str):
    """중복 결과를 CSV로 저장
    - 일반 파일 그룹: 그룹별 섹션으로 출력 (hash 8자, 용량 읽기 쉽게)
    - 아카이브 전용 그룹: 파일 쌍으로 묶어 하단에 요약 섹션으로 출력
    """
    regular_sections: list[list[list]] = []
    arc_pair_shared: dict[tuple[str, str], set[str]] = defaultdict(set)
    arc_sizes: dict[str, int] = {}
    arc_totals: dict[str, int] = {}

    for i, group in enumerate(duplicates, 1):
        arc_map: dict[str, dict] = defaultdict(lambda: {'count': 0, 'size': 0, 'hash': '', 'total': 0})
        regular_items = []

        for item in group:
            if item['type'] in ('zip_entry', 'tar_entry') and '::' in item['path']:
                arc_path = item['path'].split('::', 1)[0]
                arc_map[arc_path]['count'] += 1
                arc_map[arc_path]['size'] = max(arc_map[arc_path]['size'], item['size'])
                arc_map[arc_path]['hash'] = item.get('full_hash', item.get('hash', ''))
                arc_map[arc_path]['total'] = item.get('arc_total', 0)
            else:
                regular_items.append(item)

        for arc_path, info in arc_map.items():
            if arc_path not in arc_sizes:
                try:
                    arc_sizes[arc_path] = os.path.getsize(arc_path)
                except OSError:
                    arc_sizes[arc_path] = 0
            arc_totals[arc_path] = info['total']

        if regular_items:
            rows = []
            for item in regular_items:
                rows.append([
                    i,
                    item.get('full_hash', item.get('hash', ''))[:8],
                    item['path'],
                    _fmt_size(item['size']),
                    item['type'],
                    '', '',
                    item.get('created', ''),
                    '',
                ])
            for arc_path, info in arc_map.items():
                rows.append([
                    i,
                    info['hash'][:8] if info['hash'] else '',
                    arc_path,
                    _fmt_size(arc_sizes[arc_path]),
                    'archive',
                    info['count'],
                    info['total'],
                    '',  # archive는 created 없음
                    '',
                ])
            regular_sections.append(rows)
        else:
            # 아카이브 전용 그룹 → 쌍별로 집계
            arc_list = list(arc_map.keys())
            for ia in range(len(arc_list)):
                for ib in range(ia + 1, len(arc_list)):
                    key = tuple(sorted([arc_list[ia], arc_list[ib]]))
                    file_hash = arc_map[arc_list[ia]]['hash']
                    if file_hash:
                        arc_pair_shared[key].add(file_hash)

    with open(output_path, 'w', newline='', encoding='utf-8') as f:
        writer = csv.writer(f)

        if regular_sections:
            writer.writerow(['group_id', 'hash', 'path', 'size', 'type', 'matched_inside', 'total_inside', 'created', 'keep'])
            for rows in regular_sections:
                for row in rows:
                    writer.writerow(row)
                writer.writerow([])

        if arc_pair_shared:
            writer.writerow([])
            writer.writerow(['=== 아카이브 쌍 요약 (공통 파일 내림차순) ==='])
            writer.writerow(['pair_id', '', 'path', 'size', 'type', 'shared_files', 'total_inside', 'created', 'keep'])
            sorted_pairs = sorted(arc_pair_shared.items(), key=lambda x: len(x[1]), reverse=True)
            for pid, ((arc_a, arc_b), hashes) in enumerate(sorted_pairs, 1):
                shared = len(hashes)
                for arc_path in [arc_a, arc_b]:
                    writer.writerow([
                        f'A{pid}', '',
                        arc_path,
                        _fmt_size(arc_sizes.get(arc_path, 0)),
                        'archive',
                        shared,
                        arc_totals.get(arc_path, ''),
                        '',  # archive는 created 없음
                        '',
                    ])
                writer.writerow([])

    print(f"\n[저장 완료] {output_path}")


def print_summary(duplicates: list[list[dict]]):
    """요약 출력"""
    total_groups = len(duplicates)
    total_files = sum(len(g) for g in duplicates)
    wasted_bytes = sum(
        item['size'] * (len(group) - 1)
        for group in duplicates
        for item in [group[0]]
    )

    print("\n" + "=" * 50)
    print(f"  중복 그룹 수  : {total_groups:,}개")
    print(f"  중복 파일 수  : {total_files:,}개")
    print(f"  낭비 용량     : {wasted_bytes / 1024 / 1024:.1f} MB ({wasted_bytes / 1024 / 1024 / 1024:.2f} GB)")
    print("=" * 50)



# ──────────────────────────────────────────────
# 영상 pHash (유사 영상 탐지)
# ──────────────────────────────────────────────
def get_video_duration(filepath: str) -> float | None:
    """ffprobe로 영상 길이(초) 반환"""
    try:
        result = subprocess.run(
            ['ffprobe', '-v', 'error', '-show_entries', 'format=duration',
             '-of', 'default=noprint_wrappers=1:nokey=1', filepath],
            capture_output=True, text=True, timeout=30
        )
        return float(result.stdout.strip())
    except Exception:
        return None


def extract_video_frames(filepath: str, n_frames: int = 10) -> list[object] | None:
    """
    ffmpeg로 영상에서 균등 간격 프레임 추출
    앞뒤 10% 제외 후 n_frames개 샘플링
    반환: PIL Image 리스트
    """
    if not FFMPEG_AVAILABLE or not PHASH_AVAILABLE:
        return None

    duration = get_video_duration(filepath)
    if not duration or duration < 1:
        return None

    # 앞뒤 10% 제외
    start = duration * 0.1
    end   = duration * 0.9
    span  = end - start
    if span <= 0:
        start, end, span = 0, duration, duration

    frames = []
    for i in range(n_frames):
        ts = start + span * (i / max(n_frames - 1, 1))
        try:
            result = subprocess.run(
                ['ffmpeg', '-ss', str(ts), '-i', filepath,
                 '-frames:v', '1', '-f', 'image2pipe',
                 '-vcodec', 'png', '-'],
                capture_output=True, timeout=15
            )
            if result.returncode == 0 and result.stdout:
                img = Image.open(io.BytesIO(result.stdout)).convert('RGB')
                frames.append(img)
        except Exception:
            pass

    return frames if frames else None


def compute_video_phash(filepath: str, n_frames: int = 10) -> list[str] | None:
    """영상 파일의 프레임별 pHash 리스트 반환"""
    frames = extract_video_frames(filepath, n_frames)
    if not frames:
        return None
    hashes = []
    for img in frames:
        try:
            hashes.append(str(imagehash.phash(img)))
        except Exception:
            pass
    return hashes if hashes else None


def video_sequence_distance(hashes_a: list[str], hashes_b: list[str]) -> float:
    """
    두 영상의 프레임 해시 시퀀스 간 평균 해밍 거리
    프레임 수가 다를 경우 짧은 쪽 기준으로 균등 매핑
    """
    n = min(len(hashes_a), len(hashes_b))
    if n == 0:
        return 64.0

    # 긴 쪽을 짧은 쪽 길이로 균등 다운샘플
    def resample(lst, n):
        if len(lst) == n:
            return lst
        indices = [int(i * len(lst) / n) for i in range(n)]
        return [lst[i] for i in indices]

    a = resample(hashes_a, n)
    b = resample(hashes_b, n)

    total = sum(phash_distance(ha, hb) for ha, hb in zip(a, b))
    return total / n


def find_similar_videos(
    items: list[dict],
    exact_threshold: float = 3.0,
    similar_threshold: float = 10.0,
    progress_cb=None,
) -> tuple[list, list]:
    """
    영상 pHash 시퀀스 기반 유사 영상 그룹핑
    exact_threshold : 프레임 평균 해밍 거리 ≤ 이 값 → 완전동일
    similar_threshold: 프레임 평균 해밍 거리 ≤ 이 값 → 유사
    """
    video_items = [it for it in items if it.get('video_phashes')]
    if not video_items:
        return [], []

    n = len(video_items)
    print(f"\n[영상 유사도 분석] {len(video_items)}개 영상 비교 중...")

    find_e, union_e = make_uf(n)
    find_s, union_s = make_uf(n)

    if PYBKTREE_AVAILABLE:
        def _vdist(a: tuple, b: tuple) -> int:
            return round(video_sequence_distance(a[0], b[0]) * _VDIST_SCALE)

        tree = pybktree.BKTree(_vdist)
        phashes_list = [it['video_phashes'] for it in video_items]
        for i, ph in enumerate(phashes_list):
            tree.add((ph, i))

        sim_thresh_int = round(similar_threshold * _VDIST_SCALE)
        exact_thresh_int = round(exact_threshold * _VDIST_SCALE)

        for i, ph in enumerate(phashes_list):
            for dist, (_, j) in tree.find((ph, i), sim_thresh_int):
                if j <= i:
                    continue
                if dist <= exact_thresh_int:
                    union_e(i, j)
                union_s(i, j)
            if (i + 1) % 20 == 0:
                print(f"  BK-tree 쿼리 중... {i+1}/{n}", end='\r')
                if progress_cb:
                    progress_cb(i + 1, n)
    else:  # cppbktree는 커스텀 거리함수 미지원 → O(n²) 폴백
        total_pairs = n * (n - 1) // 2
        done = 0
        for i in range(n):
            for j in range(i + 1, n):
                dist = video_sequence_distance(
                    video_items[i]['video_phashes'],
                    video_items[j]['video_phashes'],
                )
                if dist <= exact_threshold:
                    union_e(i, j)
                if dist <= similar_threshold:
                    union_s(i, j)
                done += 1
                if done % 100 == 0:
                    print(f"  비교 중... {done:,}/{total_pairs:,}", end='\r')
                    if progress_cb:
                        progress_cb(done, total_pairs)

    exact_map = defaultdict(list)
    for i, item in enumerate(video_items):
        exact_map[find_e(i)].append(item)
    exact_groups = [g for g in exact_map.values() if len(g) > 1]

    exact_paths = {it['path'] for g in exact_groups for it in g}
    similar_map = defaultdict(list)
    for i, item in enumerate(video_items):
        similar_map[find_s(i)].append(item)
    similar_groups = [
        g for g in similar_map.values()
        if len(g) > 1 and not any(it['path'] in exact_paths for it in g)
    ]

    print(f"  완전동일 그룹: {len(exact_groups)}개 / 유사 그룹: {len(similar_groups)}개      ")
    return exact_groups, similar_groups


def save_video_csv(exact_groups: list, similar_groups: list, output_path: str):
    """영상 유사도 결과 CSV 저장"""
    with open(output_path, 'w', newline='', encoding='utf-8') as f:
        writer = csv.writer(f)
        writer.writerow(['category', 'group_id', 'path', 'size_bytes', 'frame_count', 'keep'])
        for i, group in enumerate(exact_groups, 1):
            for j, item in enumerate(group):
                writer.writerow([
                    'exact', i, item['path'], item['size'],
                    len(item.get('video_phashes', [])),
                    'YES' if j == 0 else 'NO',
                ])
        for i, group in enumerate(similar_groups, 1):
            for j, item in enumerate(group):
                writer.writerow([
                    'similar', i, item['path'], item['size'],
                    len(item.get('video_phashes', [])),
                    'YES' if j == 0 else 'NO',
                ])
    print(f"[저장 완료] {output_path}")


# ──────────────────────────────────────────────
# 삭제 모드
# ──────────────────────────────────────────────
def _rule_score(path: str, prefer: list[str], reject: list[str]) -> int:
    score = 0
    for p in prefer:
        if p in path:
            score += 1
    for r in reject:
        if r in path:
            score -= 1
    return score


def delete_from_csv(
    csv_path: str,
    dry_run: bool = True,
    prefer: list[str] = [],
    reject: list[str] = [],
):
    with open(csv_path, 'r', encoding='utf-8') as f:
        reader = csv.DictReader(f)
        rows_all = list(reader)
    # type 컬럼이 있으면 file 행만, 없으면(video CSV 등) 전체 사용
    if rows_all and 'type' in rows_all[0]:
        all_rows = [r for r in rows_all if r.get('type') == 'file']
    else:
        all_rows = rows_all

    # group_id별로 묶기
    groups: dict[str, list[dict]] = defaultdict(list)
    for row in all_rows:
        gid = row.get('group_id', '').strip()
        if gid:
            groups[gid].append(row)

    to_delete: list[str] = []
    unresolved: list[tuple[str, list[dict]]] = []

    for gid, rows in groups.items():
        # CSV에 이미 keep 값이 있으면 그대로 사용
        if all(r.get('keep', '').strip().upper() in ('YES', 'NO') for r in rows):
            for r in rows:
                if r['keep'].strip().upper() == 'NO':
                    to_delete.append(r['path'])
            continue

        # 규칙 적용
        if prefer or reject:
            scored = sorted(rows, key=lambda r: _rule_score(r['path'], prefer, reject), reverse=True)
            top_score = _rule_score(scored[0]['path'], prefer, reject)
            rest_scores = [_rule_score(r['path'], prefer, reject) for r in scored[1:]]

            if top_score > (rest_scores[0] if rest_scores else top_score - 1):
                # 명확한 1위 → 나머지 삭제
                for r in scored[1:]:
                    to_delete.append(r['path'])
            else:
                unresolved.append((gid, rows))
        else:
            unresolved.append((gid, rows))

    if unresolved:
        print(f"\n[미결 그룹] {len(unresolved)}개 그룹은 규칙으로 결정할 수 없어요:")
        for gid, rows in unresolved[:10]:
            print(f"  그룹 {gid}:")
            for r in rows:
                print(f"    {r['path']}")
        if len(unresolved) > 10:
            print(f"  ... 외 {len(unresolved) - 10}개")
        print()

    if not to_delete:
        print("삭제할 파일이 없어요")
        return

    total_size = 0
    print(f"삭제 대상: {len(to_delete)}개 파일")

    for path in to_delete:
        try:
            size = os.path.getsize(path)
            total_size += size
            if dry_run:
                print(f"  [DRY RUN] {path}")
            else:
                os.remove(path)
                print(f"  삭제됨: {path}")
        except OSError as e:
            print(f"  오류: {path} → {e}")

    print(f"\n{'[DRY RUN] ' if dry_run else ''}총 {_fmt_size(total_size)} {'삭제 예정' if dry_run else '삭제 완료'}")

    if dry_run:
        print("\n실제 삭제하려면 --no-dry-run 옵션을 추가하세요:")
        print(f"  python duplicate_finder.py delete {csv_path} --no-dry-run")


# ──────────────────────────────────────────────
# 메인
# ──────────────────────────────────────────────
def _step_start(label: str) -> datetime:
    t = datetime.now()
    print(f"\n[{label}] 시작: {t.strftime('%H:%M:%S')}")
    return t


def _step_end(label: str, t: datetime) -> None:
    elapsed = datetime.now() - t
    print(f"[{label}] 완료: {datetime.now().strftime('%H:%M:%S')}  (소요 {elapsed})")


def cmd_scan(args):
    start = datetime.now()
    print(f"시작: {start.strftime('%Y-%m-%d %H:%M:%S')}")

    # 경로 유효성 검사
    for p in args.paths:
        if not os.path.isdir(p):
            print(f"[오류] 디렉토리가 아니거나 존재하지 않아요: {p}")
            sys.exit(1)

    # 파일 수집 (여러 경로)
    t_collect = _step_start("파일 탐색")
    files = []
    for p in args.paths:
        files.extend(collect_files(p))

    if len(args.paths) > 1:
        print(f"\n전체 탐색 합계: {len(files):,}개 파일")
    _step_end("파일 탐색", t_collect)

    # 압축파일 내부 수집
    if not args.no_archive:
        t_arc = _step_start("압축파일 검사")
        archive_entries = collect_archive_entries(files)
        _step_end("압축파일 검사", t_arc)
    else:
        archive_entries = []

    all_items = files + archive_entries
    print(f"\n전체 검사 대상: {len(all_items):,}개 (파일 {len(files):,} + 압축 내부 {len(archive_entries):,})")

    only_phash = getattr(args, 'only_phash', False)
    only_vhash = getattr(args, 'only_vhash', False)
    only_mode = only_phash or only_vhash

    timestamp = start.strftime('%Y%m%d_%H%M%S')

    # 해시 기반 중복 탐지 (only 모드일 때 스킵)
    duplicates = []
    dup_skip_paths: set[str] = set()
    if not only_mode:
        t_hash = _step_start("해시 계산")
        duplicates = find_duplicates(all_items)
        dup_skip_paths = {item['path'] for group in duplicates for item in group[1:]}
        if dup_skip_paths:
            print(f"  pHash 대상 제외: 해시 중복 확인 {len(dup_skip_paths):,}개")
        _step_end("해시 계산", t_hash)

    # 이미지 pHash 수집
    run_phash = (not args.no_phash and PHASH_AVAILABLE) and (not only_vhash)
    if run_phash:
        image_items = [
            it for it in all_items
            if (it.get('is_image') or (
                '::' in it.get('path', '') and Path(it['path'].split('::')[1]).suffix.lower() in IMAGE_EXTENSIONS
            )) and it['path'] not in dup_skip_paths
        ]
        t_phash = _step_start(f"이미지 pHash ({len(image_items):,}개)")
        _parallel_phash(image_items)
        print(f"  pHash 수집 완료      ")
        _step_end("이미지 pHash", t_phash)
    elif not PHASH_AVAILABLE and not args.no_phash and not only_vhash:
        print("\n[pHash] imagehash/Pillow 미설치 → 이미지 유사도 검사 건너뜀")
        print("  설치: pip install imagehash Pillow")

    # 영상 pHash 수집
    run_vhash = (not args.no_vhash and FFMPEG_AVAILABLE and PHASH_AVAILABLE) and (not only_phash)
    if run_vhash:
        video_items = [it for it in all_items if it.get('is_video') and it['path'] not in dup_skip_paths]
        if video_items:
            t_vhash = _step_start(f"영상 pHash ({len(video_items):,}개, 파일당 {args.vhash_frames}프레임)")
            print(f"  주의: 영상 수에 따라 시간이 오래 걸릴 수 있어요")
            _parallel_vphash(video_items, n_frames=args.vhash_frames)
            done_count = sum(1 for it in video_items if it.get('video_phashes'))
            print(f"  영상 pHash 수집 완료: {done_count}/{len(video_items)}개      ")
            _step_end("영상 pHash", t_vhash)
    elif not FFMPEG_AVAILABLE and not args.no_vhash and not only_phash:
        print("\n[영상 pHash] ffmpeg 미설치 → 영상 유사도 검사 건너뜀")
        print("  설치: brew install ffmpeg  (macOS)")

    # 일반 중복 결과 저장
    if not only_mode:
        if duplicates:
            output = args.output or f"duplicates_{timestamp}.csv"
            save_csv(duplicates, output)
            print_summary(duplicates)
            print(f"\nCSV 파일을 열어서 keep 컬럼을 수정한 후 delete 명령으로 삭제하세요:")
            print(f"  python duplicate_finder.py delete {output}")
        else:
            print("\n중복 파일 없음!")

    # 이미지 유사도 결과 저장
    if run_phash:
        t_img_sim = _step_start("이미지 유사도 분석")
        exact_groups, similar_groups = find_similar_images(
            all_items,
            exact_threshold=args.phash_exact,
            similar_threshold=args.phash_similar,
        )
        _step_end("이미지 유사도 분석", t_img_sim)
        if exact_groups or similar_groups:
            img_output = f"image_similar_{timestamp}.csv"
            save_image_csv(exact_groups, similar_groups, img_output)
            print(f"\n이미지 완전동일 그룹: {len(exact_groups)}개 / 유사 그룹: {len(similar_groups)}개")
            print(f"  (기준: exact≤{args.phash_exact}, similar≤{args.phash_similar})")
        else:
            print("\n유사 이미지 없음")

    # 영상 유사도 결과 저장
    if run_vhash:
        t_vid_sim = _step_start("영상 유사도 분석")
        v_exact, v_similar = find_similar_videos(
            all_items,
            exact_threshold=args.vhash_exact,
            similar_threshold=args.vhash_similar,
        )
        _step_end("영상 유사도 분석", t_vid_sim)
        if v_exact or v_similar:
            vid_output = f"video_similar_{timestamp}.csv"
            save_video_csv(v_exact, v_similar, vid_output)
            print(f"\n영상 완전동일 그룹: {len(v_exact)}개 / 유사 그룹: {len(v_similar)}개")
            print(f"  (기준: exact≤{args.vhash_exact}, similar≤{args.vhash_similar}, {args.vhash_frames}프레임)")
        else:
            print("\n유사 영상 없음")

    # 압축파일 간 겹침 분석 (only 모드일 때 스킵)
    if not only_mode and archive_entries and args.min_overlap > 0:
        t_overlap = _step_start("압축파일 겹침 분석")
        overlaps = find_archive_overlaps(archive_entries, args.min_overlap)
        if overlaps:
            overlap_output = f"archive_overlaps_{timestamp}.csv"
            save_archive_overlap_csv(overlaps, overlap_output)
            print(f"\n압축파일 겹침 쌍: {len(overlaps)}개")
            print(f"상위 5개:")
            for ov in overlaps[:5]:
                print(f"  공통 {ov['common_file_count']}개 | {os.path.basename(ov['archive_a'])} ↔ {os.path.basename(ov['archive_b'])}")
        else:
            print(f"  {args.min_overlap}개 이상 겹치는 압축파일 쌍 없음")
        _step_end("압축파일 겹침 분석", t_overlap)

    elapsed = datetime.now() - start
    end = datetime.now()
    print(f"\n종료: {end.strftime('%Y-%m-%d %H:%M:%S')}  |  총 소요: {elapsed}")


def cmd_delete(args):
    prefer = [p.strip() for p in args.prefer.split(',')] if args.prefer else []
    reject = [r.strip() for r in args.reject.split(',')] if args.reject else []

    print(f"CSV 파일: {args.csv}")
    if prefer:
        print(f"보존 우선 패턴: {prefer}")
    if reject:
        print(f"삭제 대상 패턴: {reject}")

    dry_run = not args.no_dry_run
    if dry_run:
        print("** DRY RUN 모드 (실제 삭제 없음) **")
    else:
        confirm = input("정말 삭제하시겠어요? (yes 입력): ")
        if confirm.strip().lower() != 'yes':
            print("취소됨")
            return
    delete_from_csv(args.csv, dry_run=dry_run, prefer=prefer, reject=reject)


def main():
    parser = argparse.ArgumentParser(description='NAS 중복 파일 검사 도구')
    sub = parser.add_subparsers(dest='command')

    # scan 명령
    scan_p = sub.add_parser('scan', help='중복 파일 검사')
    scan_p.add_argument('paths', nargs='+', help='검사할 디렉토리 경로 (여러 개 가능)')
    scan_p.add_argument('-o', '--output', help='출력 CSV 파일명 (기본: duplicates_날짜시간.csv)')
    scan_p.add_argument('--no-archive', action='store_true', help='압축파일 내부 검사 제외')
    scan_p.add_argument('--min-overlap', type=int, default=2,
                        help='압축파일 간 겹침 탐지 최소 파일 수 (기본: 2, 0이면 비활성화)')
    scan_p.add_argument('--no-phash', action='store_true',
                        help='이미지 pHash 유사도 검사 비활성화')
    scan_p.add_argument('--phash-exact', type=int, default=0,
                        help='완전동일 판정 해밍 거리 임계값 (기본: 0, 해상도/메타만 다른 경우)')
    scan_p.add_argument('--phash-similar', type=int, default=10,
                        help='유사 판정 해밍 거리 임계값 (기본: 10, 리사이즈/밝기 보정 등)')

    scan_p.add_argument('--no-vhash', action='store_true',
                        help='영상 pHash 유사도 검사 비활성화')
    scan_p.add_argument('--only-phash', action='store_true',
                        help='이미지 유사도 검사만 실행 (해시 중복·영상 검사 스킵)')
    scan_p.add_argument('--only-vhash', action='store_true',
                        help='영상 유사도 검사만 실행 (해시 중복·이미지 검사 스킵)')
    scan_p.add_argument('--vhash-frames', type=int, default=10,
                        help='영상당 샘플링 프레임 수 (기본: 10, 앞뒤 10%% 제외)')
    scan_p.add_argument('--vhash-exact', type=float, default=3.0,
                        help='영상 완전동일 판정 평균 해밍 거리 (기본: 3.0)')
    scan_p.add_argument('--vhash-similar', type=float, default=10.0,
                        help='영상 유사 판정 평균 해밍 거리 (기본: 10.0)')

    # delete 명령
    del_p = sub.add_parser('delete', help='CSV 기반 중복 파일 삭제')
    del_p.add_argument('csv', help='scan으로 생성된 CSV 파일')
    del_p.add_argument('--no-dry-run', action='store_true', help='실제 삭제 실행 (기본은 dry-run)')
    del_p.add_argument('--prefer', default='', help='보존 우선 경로 패턴 (쉼표 구분, 예: "Sub1,photos")')
    del_p.add_argument('--reject', default='', help='삭제 대상 경로 패턴 (쉼표 구분, 예: ".@__thumb,backup")')

    args = parser.parse_args()

    if args.command == 'scan':
        cmd_scan(args)
    elif args.command == 'delete':
        cmd_delete(args)
    else:
        parser.print_help()


if __name__ == '__main__':
    main()
