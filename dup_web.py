#!/usr/bin/env python3
"""
dup_web.py
중복 파일 탐지 웹 UI

사용법:
  python dup_web.py              # 기본 실행 (포트 8765)
  python dup_web.py --verbose    # 상세 로그 출력
  python dup_web.py --port 9000  # 포트 변경
"""

import os
import sys
import json
import shutil
import asyncio
import argparse
import subprocess
import webbrowser
from collections import defaultdict
from datetime import datetime
from pathlib import Path
from typing import Optional

from fastapi import FastAPI, HTTPException
from fastapi.responses import HTMLResponse, JSONResponse
from fastapi.staticfiles import StaticFiles
from pydantic import BaseModel
import uvicorn

# duplicate_finder 함수 임포트
sys.path.insert(0, str(Path(__file__).parent))
from duplicate_finder import (
    collect_files,
    collect_archive_entries,
    find_duplicates,
    find_similar_images,
    find_similar_videos,
    find_archive_overlaps,
    make_uf,
    PHASH_AVAILABLE,
    FFMPEG_AVAILABLE,
)

# ──────────────────────────────────────────────
# 전역 상태
# ──────────────────────────────────────────────

from contextlib import asynccontextmanager

@asynccontextmanager
async def lifespan(app):
    yield
    global _scan_process
    if _scan_process and _scan_process.is_alive():
        _scan_process.kill()
        _scan_process.join(timeout=3)
        _scan_process = None

app = FastAPI(title="Dup Web", lifespan=lifespan)
VERBOSE = False

import multiprocessing
import uuid as _uuid_mod
_scan_process: multiprocessing.Process | None = None

# 스캔 결과 저장소
scan_state: dict = {
    "status": "idle",       # idle | scanning | done | error | cancelled
    "log": [],
    "result": None,         # groups dict
    "timestamp": None,
    "paths": [],
    "session_uuid": None,   # 스캔/로드 세션 식별자
}


# ──────────────────────────────────────────────
# 스캔 결과 → JSON 변환 (Union-Find 아카이브 병합)
# ──────────────────────────────────────────────

def _fmt_size(n: int) -> str:
    for unit in ("B", "KB", "MB", "GB"):
        if n < 1024:
            return f"{n:.1f} {unit}"
        n /= 1024
    return f"{n:.1f} TB"


def _build_regular_groups(duplicates: list) -> list:
    groups = []
    for gid, group in enumerate(duplicates, 1):
        files = []
        for i, item in enumerate(group):
            files.append({
                "path": item["path"],
                "size": item["size"],
                "size_fmt": _fmt_size(item["size"]),
                "type": item.get("type", "file"),
                "hash": item.get("full_hash", item.get("hash", ""))[:8],
                "created": item.get("created", ""),
                "keep": True,
            })
        savable = sum(f["size"] for f in files[1:])
        groups.append({
            "id": f"R{gid}",
            "files": files,
            "savable_fmt": _fmt_size(savable),
        })
    return groups


def _build_image_groups(exact_groups: list, similar_groups: list) -> list:
    groups = []
    gid = 1
    for category, grp_list in [("exact", exact_groups), ("similar", similar_groups)]:
        for group in grp_list:
            files = []
            for i, item in enumerate(group):
                files.append({
                    "path": item["path"],
                    "size": item["size"],
                    "size_fmt": _fmt_size(item["size"]),
                    "type": item.get("type", "file"),
                    "phash": item.get("phash", ""),
                    "keep": True,
                })
            savable = sum(f["size"] for f in files[1:])
            groups.append({
                "id": f"I{gid}",
                "category": category,
                "files": files,
                "savable_fmt": _fmt_size(savable),
            })
            gid += 1
    return groups


def _build_video_groups(exact_groups: list, similar_groups: list) -> list:
    groups = []
    gid = 1
    for category, grp_list in [("exact", exact_groups), ("similar", similar_groups)]:
        for group in grp_list:
            files = []
            for i, item in enumerate(group):
                files.append({
                    "path": item["path"],
                    "size": item["size"],
                    "size_fmt": _fmt_size(item["size"]),
                    "frame_count": len(item.get("video_phashes", [])),
                    "keep": True,
                })
            savable = sum(f["size"] for f in files[1:])
            groups.append({
                "id": f"V{gid}",
                "category": category,
                "files": files,
                "savable_fmt": _fmt_size(savable),
            })
            gid += 1
    return groups


def _build_archive_groups(overlaps: list) -> list:
    """페어 목록을 Union-Find로 병합해 그룹화"""
    if not overlaps:
        return []

    all_paths = list({p for ov in overlaps for p in (ov["archive_a"], ov["archive_b"])})
    path_idx = {p: i for i, p in enumerate(all_paths)}
    find, union = make_uf(len(all_paths))

    for ov in overlaps:
        union(path_idx[ov["archive_a"]], path_idx[ov["archive_b"]])

    # 그룹별 집계
    group_data: dict[int, dict] = defaultdict(lambda: {
        "paths": set(), "shared": 0, "sizes": {}
    })
    for p in all_paths:
        root = find(path_idx[p])
        group_data[root]["paths"].add(p)
        try:
            group_data[root]["sizes"][p] = os.path.getsize(p)
        except OSError:
            group_data[root]["sizes"][p] = 0

    for ov in overlaps:
        root = find(path_idx[ov["archive_a"]])
        group_data[root]["shared"] = max(group_data[root]["shared"], ov["common_file_count"])

    # 공통 파일 수 내림차순 정렬
    sorted_groups = sorted(group_data.values(), key=lambda x: x["shared"], reverse=True)

    groups = []
    for gid, gdata in enumerate(sorted_groups, 1):
        paths_sorted = sorted(gdata["paths"], key=lambda p: gdata["sizes"].get(p, 0), reverse=True)
        files = []
        for i, p in enumerate(paths_sorted):
            sz = gdata["sizes"].get(p, 0)
            files.append({
                "path": p,
                "size": sz,
                "size_fmt": _fmt_size(sz),
                "type": "archive",
                "shared": gdata["shared"],
                "keep": True,
            })
        savable = sum(f["size"] for f in files[1:])
        groups.append({
            "id": f"A{gid}",
            "shared": gdata["shared"],
            "files": files,
            "savable_fmt": _fmt_size(savable),
        })
    return groups


# ──────────────────────────────────────────────
# 스캔 실행 (백그라운드)
# ──────────────────────────────────────────────

def _log(msg: str):
    print(msg, flush=True)
    scan_state["log"].append(msg)


def _scan_worker(paths: list[str], options: dict, queue: multiprocessing.Queue):
    """별도 프로세스에서 실행되는 스캔 워커. 결과를 queue로 전송."""
    _do_scan(paths, options, queue)


async def run_scan(paths: list[str], options: dict):
    global _scan_process
    scan_state["status"] = "scanning"
    scan_state["log"] = []
    scan_state["result"] = None
    scan_state["paths"] = paths
    scan_state["session_uuid"] = str(_uuid_mod.uuid4())

    queue: multiprocessing.Queue = multiprocessing.Queue()

    _scan_process = multiprocessing.Process(
        target=_scan_worker, args=(paths, options, queue), daemon=True
    )
    _scan_process.start()

    # 메인 프로세스에서 queue 폴링
    loop = asyncio.get_event_loop()
    await loop.run_in_executor(None, _collect_results, queue)


def _collect_results(queue: multiprocessing.Queue):
    """워커 프로세스로부터 메시지를 받아 scan_state를 갱신."""
    import queue as _queue
    while True:
        try:
            msg = queue.get(timeout=0.3)
        except _queue.Empty:
            if _scan_process and not _scan_process.is_alive():
                # 프로세스가 결과 없이 종료 → 취소된 것
                if scan_state["status"] == "scanning":
                    scan_state["status"] = "cancelled"
                    _log("중단됨")
                break
            continue

        kind = msg.get("kind")
        if kind == "log":
            _log(msg["text"])
        elif kind == "result":
            scan_state["result"] = msg["data"]
            scan_state["timestamp"] = msg.get("timestamp")
            scan_state["status"] = "done"
            break
        elif kind == "error":
            _log(f"\n[오류] {msg['text']}")
            scan_state["status"] = "error"
            break


def _do_scan(paths: list[str], options: dict, queue: multiprocessing.Queue):
    def log(msg: str):
        print(msg, flush=True)
        queue.put({"kind": "log", "text": msg})

    try:
        start = datetime.now()
        timestamp = start.strftime("%Y%m%d_%H%M%S")
        log(f"스캔 시작: {start.strftime('%Y-%m-%d %H:%M:%S')}")
        log(f"대상 경로: {', '.join(paths)}")

        # 파일 수집
        log("\n[1/5] 파일 탐색 중...")
        files = []
        _last_collect_log = [0]
        def _collect_progress(count, dirpath):
            if count - _last_collect_log[0] >= 1000:
                _last_collect_log[0] = count
                log(f"  탐색 중... {count:,}개 ({dirpath})")
        for p in paths:
            collected = collect_files(p, progress_cb=_collect_progress)
            files.extend(collected)
            log(f"  {p}: {len(collected):,}개")
        log(f"  합계: {len(files):,}개 파일")

        # 압축파일 내부
        archive_entries = []
        if not options.get("no_archive"):
            log("\n[2/5] 압축파일 내부 검사 중...")
            arc_total_count = sum(
                1 for f in files
                if Path(f['path']).suffix.lower() in {'.zip', '.gz', '.tgz', '.tar', '.bz2'}
            )
            log(f"  압축파일 {arc_total_count:,}개 검사 시작")

            def _arc_progress(done, total, name):
                if done % 10 == 0 or done == total:
                    log(f"  ({done}/{total}) {name}")

            archive_entries = collect_archive_entries(files, progress_cb=_arc_progress)
            log(f"  압축 내부 항목: {len(archive_entries):,}개")

        all_items = files + archive_entries

        # 해시 기반 중복
        log("\n[3/5] 해시 기반 중복 탐지 중...")
        _last_hash_log = {'partial': 0, 'full': 0}
        def _hash_progress(stage, done, total):
            key = stage
            if done - _last_hash_log[key] >= 500 or done == total:
                _last_hash_log[key] = done
                label = '부분' if stage == 'partial' else '전체'
                log(f"  {label} 해시: {done:,}/{total:,}")
        duplicates = find_duplicates(all_items, progress_cb=_hash_progress)
        dup_skip_paths = {item["path"] for group in duplicates for item in group[1:]}
        log(f"  중복 그룹: {len(duplicates)}개 ({len(dup_skip_paths):,}개 파일)")

        # 이미지 유사도
        exact_img, similar_img = [], []
        if not options.get("no_phash") and PHASH_AVAILABLE:
            log("\n[4/5] 이미지 유사도 분석 중...")
            image_items = [
                it for it in all_items
                if (it.get("is_image") or (
                    "::" in it.get("path", "") and
                    Path(it["path"].split("::")[1]).suffix.lower() in
                    {".jpg", ".jpeg", ".png", ".gif", ".bmp", ".webp", ".tiff", ".heic"}
                )) and it["path"] not in dup_skip_paths
            ]
            log(f"  이미지 {len(image_items):,}개 분석")
            from duplicate_finder import _parallel_phash
            _last_phash_log = [0]
            def _phash_progress(done, total):
                if done - _last_phash_log[0] >= 200 or done == total:
                    _last_phash_log[0] = done
                    log(f"  이미지 pHash: {done:,}/{total:,}")
            _parallel_phash(image_items, progress_cb=_phash_progress)
            _last_img_cmp_log = [0]
            def _img_cmp_progress(done, total):
                if done - _last_img_cmp_log[0] >= 10000 or done == total:
                    _last_img_cmp_log[0] = done
                    log(f"  이미지 비교: {done:,}/{total:,}")
            exact_img, similar_img = find_similar_images(
                all_items,
                exact_threshold=options.get("phash_exact", 0),
                similar_threshold=options.get("phash_similar", 10),
                progress_cb=_img_cmp_progress,
            )
            log(f"  exact: {len(exact_img)}그룹 / similar: {len(similar_img)}그룹")
        else:
            log("\n[4/5] 이미지 유사도 분석 건너뜀")

        # 영상 유사도
        exact_vid, similar_vid = [], []
        if not options.get("no_vhash") and FFMPEG_AVAILABLE and PHASH_AVAILABLE:
            log("\n[5/5] 영상 유사도 분석 중...")
            video_items = [it for it in all_items if it.get("is_video") and it["path"] not in dup_skip_paths]
            log(f"  영상 {len(video_items):,}개 분석")
            if video_items:
                from duplicate_finder import _parallel_vphash
                def _vphash_progress(done, total, name):
                    if done % 5 == 0 or done == total:
                        log(f"  영상 pHash ({done}/{total}): {name}")
                _parallel_vphash(video_items, n_frames=options.get("vhash_frames", 10), progress_cb=_vphash_progress)
                _last_vid_cmp_log = [0]
                def _vid_cmp_progress(done, total):
                    if done - _last_vid_cmp_log[0] >= 50 or done == total:
                        _last_vid_cmp_log[0] = done
                        log(f"  영상 비교: {done:,}/{total:,}")
                exact_vid, similar_vid = find_similar_videos(
                    all_items,
                    exact_threshold=options.get("vhash_exact", 0),
                    similar_threshold=options.get("vhash_similar", 10),
                    progress_cb=_vid_cmp_progress,
                )
                log(f"  exact: {len(exact_vid)}그룹 / similar: {len(similar_vid)}그룹")
        else:
            log("\n[5/5] 영상 유사도 분석 건너뜀")

        # 아카이브 겹침
        overlaps = []
        if archive_entries and not options.get("no_archive"):
            min_overlap = options.get("min_overlap", 5)
            min_arc_files = options.get("min_arc_files", 0)
            overlaps = find_archive_overlaps(archive_entries, min_overlap, min_arc_files=min_arc_files)
            log(f"\n아카이브 겹침 쌍: {len(overlaps)}개 (겹침≥{min_overlap}" + (f", 파일수≥{min_arc_files}" if min_arc_files else "") + ")")

        # 결과 빌드
        result = {
            "regular": _build_regular_groups(duplicates),
            "image": _build_image_groups(exact_img, similar_img),
            "video": _build_video_groups(exact_vid, similar_vid),
            "archive": _build_archive_groups(overlaps),
        }

        elapsed = (datetime.now() - start).total_seconds()
        log(f"\n완료! ({elapsed:.1f}초)")
        log(f"  일반: {len(result['regular'])}그룹 | 이미지: {len(result['image'])}그룹 | 영상: {len(result['video'])}그룹 | 아카이브: {len(result['archive'])}그룹")
        queue.put({"kind": "result", "data": result, "timestamp": timestamp})

    except Exception as e:
        import traceback
        queue.put({"kind": "error", "text": f"{e}\n{traceback.format_exc()}"})


# ──────────────────────────────────────────────
# API
# ──────────────────────────────────────────────

class ScanRequest(BaseModel):
    paths: list[str]
    no_phash: bool = False
    no_vhash: bool = False
    no_archive: bool = False
    phash_exact: int = 0
    phash_similar: int = 10
    vhash_exact: int = 0
    vhash_similar: int = 10
    vhash_frames: int = 10
    min_overlap: int = 5
    min_arc_files: int = 0  # 0 = 제한 없음


class DeleteRequest(BaseModel):
    paths: list[str]


class SaveCsvRequest(BaseModel):
    state: dict  # {regular: [...], image: [...], video: [...], archive: [...]}


@app.post("/api/scan")
async def api_scan(req: ScanRequest):
    if scan_state["status"] == "scanning":
        raise HTTPException(status_code=409, detail="스캔이 이미 진행 중입니다")

    for p in req.paths:
        if not os.path.isdir(p):
            raise HTTPException(status_code=400, detail=f"디렉토리가 존재하지 않아요: {p}")

    options = req.model_dump()
    options.pop("paths")
    asyncio.create_task(run_scan(req.paths, options))
    return {"status": "started"}


@app.get("/api/scan/status")
async def api_scan_status():
    return {
        "status": scan_state["status"],
        "log": scan_state["log"][-50:],
        "result": scan_state["result"],
        "timestamp": scan_state["timestamp"],
        "paths": scan_state["paths"],
        "session_uuid": scan_state["session_uuid"],
    }


@app.post("/api/scan/cancel")
async def api_scan_cancel():
    global _scan_process
    if scan_state["status"] != "scanning":
        raise HTTPException(status_code=409, detail="스캔 중이 아닙니다")
    if _scan_process and _scan_process.is_alive():
        _scan_process.kill()  # SIGKILL — terminate(SIGTERM)은 ThreadPoolExecutor에 무시됨
        _scan_process.join(timeout=3)
        _scan_process = None
    scan_state["status"] = "cancelled"
    _log("중단됨")
    return {"status": "cancelled"}


@app.post("/api/open-finder")
async def api_open_finder(body: dict):
    path = body.get("path", "")
    if not path:
        raise HTTPException(status_code=400, detail="path 필요")
    subprocess.Popen(["open", "-R", path])
    return {"ok": True}


@app.post("/api/check-files")
async def api_check_files():
    """scan_state의 모든 탭에서 실제 존재하지 않는 파일을 제거하고 missing 경로 목록 반환."""
    if not scan_state.get("result"):
        raise HTTPException(status_code=400, detail="스캔 결과가 없습니다")
    _log("\n[파일 확인] 존재 여부 확인 시작...")
    missing = []
    for tab in ("regular", "image", "video", "archive"):
        groups = scan_state["result"].get(tab) or []
        new_groups = []
        for g in groups:
            kept = [f for f in g["files"] if os.path.exists(f["path"])]
            gone = [f["path"] for f in g["files"] if not os.path.exists(f["path"])]
            missing.extend(gone)
            if len(kept) > 1:
                new_groups.append({**g, "files": kept})
        scan_state["result"][tab] = new_groups
    if missing:
        for p in missing:
            _log(f"  ✗ {p}")
        _log(f"[파일 확인 완료] {len(missing)}개 없는 파일 제거됨")
    else:
        _log("[파일 확인 완료] 모든 파일이 존재합니다")
    return {"missing": missing, "count": len(missing)}


@app.post("/api/delete")
async def api_delete(req: DeleteRequest):
    deleted = []
    errors = []
    _log(f"\n[삭제] {len(req.paths)}개 파일 삭제 시작")
    for path in req.paths:
        try:
            if os.path.isfile(path):
                os.remove(path)
                deleted.append(path)
                _log(f"  ✓ {path}")
            else:
                errors.append({"path": path, "error": "파일이 존재하지 않음"})
                _log(f"  ✗ {path} (없음)")
        except Exception as e:
            errors.append({"path": path, "error": str(e)})
            _log(f"  ✗ {path} ({e})")
    _log(f"[삭제 완료] {len(deleted)}개 성공" + (f" / {len(errors)}개 실패" if errors else ""))

    # scan_state["result"]에서도 삭제된 경로 제거 (리프레시 후 복원 시 반영)
    if scan_state.get("result") and deleted:
        deleted_set = set(deleted)
        for tab in ("regular", "image", "video", "archive"):
            groups = scan_state["result"].get(tab) or []
            groups = [
                {**g, "files": [f for f in g["files"] if f["path"] not in deleted_set]}
                for g in groups
            ]
            scan_state["result"][tab] = [g for g in groups if len(g["files"]) > 1]

    return {"deleted": deleted, "errors": errors}


def _groups_to_csv_rows(tab: str, groups: list) -> list:
    """groups → CSV rows (헤더 포함, 빈 행으로 그룹 구분)."""
    rows = []
    if tab == "regular":
        rows.append(["group_id", "path", "size", "type", "hash", "created", "keep"])
        for g in groups:
            for f in g["files"]:
                rows.append([g["id"], f["path"], f["size"], f.get("type", "file"),
                              f.get("hash", ""), f.get("created", ""), "YES" if f["keep"] else "NO"])
            rows.append([])
    elif tab == "image":
        rows.append(["category", "group_id", "path", "size_bytes", "type", "phash", "keep"])
        for g in groups:
            for f in g["files"]:
                rows.append([g.get("category", ""), g["id"], f["path"], f["size"],
                              f.get("type", "file"), f.get("phash", ""), "YES" if f["keep"] else "NO"])
            rows.append([])
    elif tab == "video":
        rows.append(["category", "group_id", "path", "size_bytes", "frame_count", "keep"])
        for g in groups:
            for f in g["files"]:
                rows.append([g.get("category", ""), g["id"], f["path"], f["size"],
                              f.get("frame_count", ""), "YES" if f["keep"] else "NO"])
            rows.append([])
    elif tab == "archive":
        rows.append(["group_id", "path", "size", "shared_files", "keep"])
        for g in groups:
            for f in g["files"]:
                rows.append([g["id"], f["path"], f["size"], f.get("shared", ""), "YES" if f["keep"] else "NO"])
            rows.append([])
    return rows


def _csv_rows_to_groups(tab: str, rows: list) -> list:
    """CSV rows(dict) → groups list."""
    groups_map: dict = {}
    for row in rows:
        gid = row.get("group_id", "").strip()
        if not gid:
            continue
        if gid not in groups_map:
            groups_map[gid] = {"id": gid, "files": [], "savable_fmt": "", "category": row.get("category", "")}
            if tab == "archive":
                groups_map[gid]["shared"] = 0
        try:
            size = int(row.get("size") or row.get("size_bytes") or 0)
        except ValueError:
            size = 0
        keep = (row.get("keep", "YES").strip().upper() != "NO")
        f: dict = {"path": row.get("path", ""), "size": size, "size_fmt": _fmt_size(size), "keep": keep}
        if tab == "regular":
            f["type"] = row.get("type", "file")
            f["hash"] = row.get("hash", "")
            f["created"] = row.get("created", "")
        elif tab == "image":
            f["type"] = row.get("type", "file")
            f["phash"] = row.get("phash", "")
        elif tab == "video":
            f["frame_count"] = row.get("frame_count", "")
        elif tab == "archive":
            f["type"] = "archive"
            try:
                shared = int(row.get("shared_files") or 0)
            except ValueError:
                shared = 0
            f["shared"] = shared
            groups_map[gid]["shared"] = max(groups_map[gid].get("shared", 0), shared)
        groups_map[gid]["files"].append(f)
    groups = list(groups_map.values())
    for g in groups:
        savable = sum(f["size"] for f in g["files"] if not f["keep"])
        g["savable_fmt"] = _fmt_size(savable)
    return groups


@app.post("/api/save-csv")
async def api_save_csv(req: SaveCsvRequest):
    """현재 state 전체를 ZIP으로 저장. 각 탭이 개별 CSV + manifest.json 포함."""
    import csv as csv_mod
    import zipfile
    import io

    session_uuid = scan_state.get("session_uuid") or str(_uuid_mod.uuid4())
    timestamp = scan_state.get("timestamp") or datetime.now().strftime("%Y%m%d_%H%M%S")
    zip_name = f"dup_session_{timestamp}.zip"
    zip_path = Path.cwd() / zip_name

    tab_filenames = {
        "regular": "duplicates.csv",
        "image": "image_similar.csv",
        "video": "video_similar.csv",
        "archive": "archive_overlaps.csv",
    }

    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED) as zf:
        # manifest
        manifest = {
            "session_uuid": session_uuid,
            "timestamp": timestamp,
            "tabs": {},
            "warning": "이 ZIP 파일 내의 CSV를 임의로 수정하면 session_uuid 불일치 또는 데이터 손상으로 불러오기가 실패할 수 있습니다.",
        }
        for tab, filename in tab_filenames.items():
            groups = req.state.get(tab) or []
            if not groups:
                continue
            rows = _groups_to_csv_rows(tab, groups)
            buf = io.StringIO()
            writer = csv_mod.writer(buf)
            for row in rows:
                writer.writerow(row)
            zf.writestr(filename, buf.getvalue())
            manifest["tabs"][tab] = {"file": filename, "groups": len(groups)}
        zf.writestr("manifest.json", json.dumps(manifest, ensure_ascii=False, indent=2))

    return {"filename": str(zip_path), "session_uuid": session_uuid}


class LoadCsvRequest(BaseModel):
    path: str  # ZIP 파일 경로


@app.post("/api/load-csv")
async def api_load_csv(req: LoadCsvRequest):
    """ZIP 파일을 읽어 모든 탭 state 복원. session_uuid 검증 포함."""
    import csv as csv_mod
    import zipfile

    zip_path = Path(req.path)
    if not zip_path.exists():
        raise HTTPException(status_code=404, detail=f"파일 없음: {req.path}")
    if zip_path.suffix.lower() != ".zip":
        raise HTTPException(status_code=400, detail="ZIP 파일만 불러올 수 있습니다")

    tab_filenames = {
        "duplicates.csv": "regular",
        "image_similar.csv": "image",
        "video_similar.csv": "video",
        "archive_overlaps.csv": "archive",
    }

    try:
        with zipfile.ZipFile(zip_path, "r") as zf:
            names = zf.namelist()

            # manifest 읽기
            if "manifest.json" not in names:
                raise HTTPException(status_code=400, detail="manifest.json 없음 — dup_web에서 저장한 ZIP이 아닙니다")
            manifest = json.loads(zf.read("manifest.json").decode("utf-8"))
            session_uuid = manifest.get("session_uuid")
            if not session_uuid:
                raise HTTPException(status_code=400, detail="manifest에 session_uuid가 없습니다")

            # 각 탭 CSV 파싱
            result: dict = {"regular": [], "image": [], "video": [], "archive": []}
            loaded_tabs = []
            for csv_name, tab in tab_filenames.items():
                if csv_name not in names:
                    continue
                content = zf.read(csv_name).decode("utf-8")
                reader = csv_mod.DictReader(content.splitlines())
                rows = [r for r in reader if any(v.strip() for v in r.values())]
                result[tab] = _csv_rows_to_groups(tab, rows)
                loaded_tabs.append(tab)

    except zipfile.BadZipFile:
        raise HTTPException(status_code=400, detail="유효하지 않은 ZIP 파일입니다")

    # scan_state 업데이트 (리프레시 후에도 복원 가능)
    scan_state["result"] = result
    scan_state["session_uuid"] = session_uuid
    scan_state["timestamp"] = manifest.get("timestamp")
    scan_state["status"] = "done"

    counts = {tab: len(result[tab]) for tab in loaded_tabs}
    return {"session_uuid": session_uuid, "tabs": loaded_tabs, "counts": counts}


# ──────────────────────────────────────────────
# 프론트엔드 HTML
# ──────────────────────────────────────────────

HTML = r"""<!DOCTYPE html>
<html lang="ko">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Dup Web</title>
<style>
  :root {
    --bg: #1a1a1a; --surface: #252525; --surface2: #2e2e2e;
    --border: #3a3a3a; --text: #e0e0e0; --text2: #999;
    --accent: #4a9eff; --green: #4caf50; --red: #f44336;
    --yellow: #ff9800; --radius: 6px;
  }
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body { background: var(--bg); color: var(--text); font: 13px/1.5 'SF Mono', monospace; }

  /* 레이아웃 */
  #app { display: flex; flex-direction: column; height: 100vh; }
  #header { padding: 12px 16px; background: var(--surface); border-bottom: 1px solid var(--border); display: flex; align-items: center; gap: 12px; }
  #header h1 { font-size: 15px; font-weight: 600; color: var(--accent); }
  #main { flex: 1; display: flex; overflow: hidden; }
  #sidebar { width: 320px; min-width: 220px; border-right: 1px solid var(--border); display: flex; flex-direction: column; }
  #content { flex: 1; display: flex; flex-direction: column; overflow: hidden; padding: 0; }
  #groups-container-wrap { flex: 1; overflow-y: auto; }

  /* 사이드바 */
  #scan-panel { padding: 12px; border-bottom: 1px solid var(--border); }
  #scan-panel label { display: block; font-size: 11px; color: var(--text2); margin-bottom: 4px; }
  #paths-input { width: 100%; background: var(--bg); border: 1px solid var(--border); color: var(--text); border-radius: var(--radius); padding: 6px 8px; font: inherit; resize: vertical; min-height: 60px; }
  #paths-input:focus { outline: none; border-color: var(--accent); }
  .opt-row { display: flex; gap: 8px; margin-top: 8px; flex-wrap: wrap; }
  .opt-row label { display: flex; align-items: center; gap: 4px; font-size: 11px; color: var(--text2); cursor: pointer; }
  #scan-btn { margin-top: 10px; width: 100%; padding: 7px; background: var(--accent); color: #fff; border: none; border-radius: var(--radius); font: inherit; font-weight: 600; cursor: pointer; }
  #scan-btn:disabled { opacity: 0.5; cursor: not-allowed; }
  #scan-btn:hover:not(:disabled) { filter: brightness(1.15); }

  /* 탭 */
  #tabs { display: flex; border-bottom: 1px solid var(--border); background: var(--surface); }
  .tab { padding: 8px 14px; cursor: pointer; font-size: 12px; color: var(--text2); border-bottom: 2px solid transparent; white-space: nowrap; }
  .tab.active { color: var(--accent); border-bottom-color: var(--accent); }
  .tab .badge { display: inline-block; background: var(--surface2); border-radius: 10px; padding: 1px 6px; font-size: 10px; margin-left: 4px; }

  /* 필터 바 */
  #filter-bar { padding: 8px 12px; border-bottom: 1px solid var(--border); display: flex; gap: 8px; align-items: center; background: var(--surface); flex-wrap: wrap; }
  #filter-input { flex: 1; min-width: 140px; background: var(--bg); border: 1px solid var(--border); color: var(--text); border-radius: var(--radius); padding: 4px 8px; font: inherit; }
  #filter-input:focus { outline: none; border-color: var(--accent); }
  .filter-btn { padding: 4px 10px; background: var(--surface2); border: 1px solid var(--border); color: var(--text2); border-radius: var(--radius); font: inherit; font-size: 11px; cursor: pointer; white-space: nowrap; }
  .filter-btn:hover { color: var(--text); border-color: var(--text2); }

  /* 그룹 리스트 */
  #groups-container { padding: 8px; }
  .group { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); margin-bottom: 8px; overflow: hidden; }
  .group-header { padding: 8px 12px; display: flex; align-items: center; gap: 8px; background: var(--surface2); cursor: pointer; user-select: none; }
  .group-header:hover { filter: brightness(1.1); }
  .group-id { font-size: 11px; color: var(--text2); min-width: 40px; flex-shrink: 0; }
  .group-meta { flex: 1; font-size: 11px; color: var(--text2); min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .group-savable { font-size: 11px; color: var(--yellow); flex-shrink: 0; margin-left: 8px; }
  .group-collapse { font-size: 11px; color: var(--text2); flex-shrink: 0; }

  /* 파일 행 */
  .file-row { display: flex; align-items: center; gap: 8px; padding: 6px 12px; border-top: 1px solid var(--border); cursor: pointer; transition: background 0.1s; outline: none; }
  .file-row:focus { box-shadow: inset 0 0 0 2px var(--accent); }
  .file-row:hover { background: var(--surface2); }
  .file-row.keep { background: rgba(76,175,80,0.08); }
  .file-row.keep:hover { background: rgba(76,175,80,0.14); }
  .file-row.remove { background: rgba(244,67,54,0.06); }
  .file-row.remove:hover { background: rgba(244,67,54,0.12); }
  .file-status { width: 16px; height: 16px; border-radius: 50%; border: 2px solid var(--border); flex-shrink: 0; }
  .file-row.keep .file-status { background: var(--green); border-color: var(--green); }
  .file-row.remove .file-status { background: var(--red); border-color: var(--red); }
  .file-info { flex: 1; min-width: 0; }
  .file-name { font-size: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; color: var(--text); }
  .file-path { font-size: 10px; color: var(--text2); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; margin-top: 1px; }
  .file-size { font-size: 11px; color: var(--text2); white-space: nowrap; flex-shrink: 0; text-align: right; min-width: 60px; }
  .finder-btn { padding: 3px 8px; background: none; border: 1px solid var(--border); color: var(--text2); border-radius: 4px; font: 11px monospace; cursor: pointer; flex-shrink: 0; white-space: nowrap; }
  .finder-btn:hover { border-color: var(--accent); color: var(--accent); }

  /* 액션 바 */
  #action-bar { padding: 8px 12px; border-top: 1px solid var(--border); background: var(--surface); display: flex; flex-direction: column; gap: 6px; }
  #action-bar .action-row { display: flex; gap: 8px; align-items: center; }
  #action-bar .action-row .info { flex: 1; font-size: 11px; color: var(--text2); }
  #action-bar .info { flex: 1; font-size: 11px; color: var(--text2); }
  .action-btn { padding: 5px 12px; border: 1px solid var(--border); border-radius: var(--radius); font: inherit; font-size: 12px; cursor: pointer; background: var(--surface2); color: var(--text); }
  .action-btn:hover { border-color: var(--text2); }
  .action-btn.danger { border-color: var(--red); color: var(--red); }
  .action-btn.danger:hover { background: rgba(244,67,54,0.1); }
  .action-btn.primary { border-color: var(--accent); color: var(--accent); }
  .action-btn.primary:hover { background: rgba(74,158,255,0.1); }

  /* 로그 패널 */
  #log-panel { flex: 1; overflow-y: auto; padding: 8px 12px; font-size: 11px; color: var(--text2); line-height: 1.7; }
  #log-panel .err { color: var(--red); }

  /* 상태 */
  #status-bar { padding: 4px 12px; font-size: 11px; color: var(--text2); border-top: 1px solid var(--border); background: var(--surface); }

  /* 빈 상태 */
  .empty { padding: 40px; text-align: center; color: var(--text2); font-size: 13px; }

  /* 모달 */
  .modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.7); display: flex; align-items: center; justify-content: center; z-index: 100; }
  .modal { background: var(--surface); border: 1px solid var(--border); border-radius: 8px; padding: 20px; min-width: 300px; max-width: 500px; }
  .modal h2 { font-size: 14px; margin-bottom: 12px; }
  .modal p { font-size: 12px; color: var(--text2); margin-bottom: 16px; }
  .modal-btns { display: flex; gap: 8px; justify-content: flex-end; }
</style>
</head>
<body>
<div id="app">
  <div id="header">
    <h1>Dup Web</h1>
    <span id="header-status" style="font-size:11px;color:var(--text2)">준비</span>
  </div>
  <div id="main">
    <!-- 사이드바 -->
    <div id="sidebar">
      <div id="scan-panel">
        <label>스캔 경로 (줄바꿈으로 여러 개)</label>
        <textarea id="paths-input" placeholder="/Users/macbook/nas/Nas/Sub3&#10;/Users/macbook/nas/Movs"></textarea>
        <div class="opt-row">
          <label><input type="checkbox" id="opt-no-phash"> 이미지 skip</label>
          <label><input type="checkbox" id="opt-no-vhash"> 영상 skip</label>
          <label><input type="checkbox" id="opt-no-archive"> 압축 skip</label>
        </div>
        <details style="margin-top:8px">
          <summary style="font-size:11px;color:var(--text2);cursor:pointer;user-select:none">고급 옵션</summary>
          <div style="margin-top:6px;display:flex;flex-direction:column;gap:5px;font-size:11px;color:var(--text2)">
            <div style="display:flex;gap:6px;align-items:center">
              <span style="flex:1">이미지 exact 임계값</span>
              <input type="number" id="opt-phash-exact" value="0" min="0" max="64" style="width:52px;background:var(--bg);border:1px solid var(--border);color:var(--text);border-radius:4px;padding:2px 4px;font:inherit;text-align:right">
            </div>
            <div style="display:flex;gap:6px;align-items:center">
              <span style="flex:1">이미지 similar 임계값</span>
              <input type="number" id="opt-phash-similar" value="10" min="0" max="64" style="width:52px;background:var(--bg);border:1px solid var(--border);color:var(--text);border-radius:4px;padding:2px 4px;font:inherit;text-align:right">
            </div>
            <div style="display:flex;gap:6px;align-items:center">
              <span style="flex:1">영상 exact 임계값</span>
              <input type="number" id="opt-vhash-exact" value="0" min="0" max="64" style="width:52px;background:var(--bg);border:1px solid var(--border);color:var(--text);border-radius:4px;padding:2px 4px;font:inherit;text-align:right">
            </div>
            <div style="display:flex;gap:6px;align-items:center">
              <span style="flex:1">영상 similar 임계값</span>
              <input type="number" id="opt-vhash-similar" value="10" min="0" max="64" style="width:52px;background:var(--bg);border:1px solid var(--border);color:var(--text);border-radius:4px;padding:2px 4px;font:inherit;text-align:right">
            </div>
            <div style="display:flex;gap:6px;align-items:center">
              <span style="flex:1">영상 샘플 프레임 수</span>
              <input type="number" id="opt-vhash-frames" value="10" min="1" max="60" style="width:52px;background:var(--bg);border:1px solid var(--border);color:var(--text);border-radius:4px;padding:2px 4px;font:inherit;text-align:right">
            </div>
            <div style="display:flex;gap:6px;align-items:center">
              <span style="flex:1">압축 최소 겹침 파일 수</span>
              <input type="number" id="opt-min-overlap" value="5" min="1" style="width:52px;background:var(--bg);border:1px solid var(--border);color:var(--text);border-radius:4px;padding:2px 4px;font:inherit;text-align:right">
            </div>
            <div style="display:flex;gap:6px;align-items:center">
              <span style="flex:1">압축 최소 내부 파일 수</span>
              <input type="number" id="opt-min-arc-files" value="0" min="0" title="0=제한없음" style="width:52px;background:var(--bg);border:1px solid var(--border);color:var(--text);border-radius:4px;padding:2px 4px;font:inherit;text-align:right">
            </div>
          </div>
        </details>
        <div style="display:flex;gap:6px;margin-top:10px">
          <button id="scan-btn" onclick="startScan()" style="flex:1;margin-top:0">스캔 시작</button>
          <button id="cancel-btn" onclick="cancelScan()" style="padding:7px 10px;background:none;border:1px solid var(--border);color:var(--text2);border-radius:var(--radius);font:inherit;cursor:pointer;display:none">중단</button>
        </div>
      </div>
      <div id="log-panel"></div>
      <div id="status-bar">대기 중</div>
    </div>

    <!-- 콘텐츠 -->
    <div id="content">
      <div id="tabs">
        <div class="tab active" data-tab="regular" onclick="switchTab('regular')">일반 <span class="badge" id="badge-regular">0</span></div>
        <div class="tab" data-tab="image" onclick="switchTab('image')">이미지 <span class="badge" id="badge-image">0</span></div>
        <div class="tab" data-tab="video" onclick="switchTab('video')">영상 <span class="badge" id="badge-video">0</span></div>
        <div class="tab" data-tab="archive" onclick="switchTab('archive')">아카이브 <span class="badge" id="badge-archive">0</span></div>
      </div>
      <div id="filter-bar">
        <input id="filter-input" type="text" placeholder="경로/파일명 필터..." oninput="applyFilter()">
        <button class="filter-btn" onclick="bulkKeep()">필터 내 일괄 KEEP</button>
        <button class="filter-btn" onclick="bulkRemove()">필터 내 일괄 REMOVE</button>
        <button class="filter-btn" onclick="clearFilter()">초기화</button>
      </div>
      <div id="groups-container-wrap"><div id="groups-container"></div></div>
      <div id="action-bar">
        <div class="action-row">
          <input id="load-csv-input" type="text" placeholder="dup_session_*.zip 경로..."
            style="flex:1;min-width:0;background:var(--bg);border:1px solid var(--border);color:var(--text);border-radius:var(--radius);padding:4px 8px;font:inherit;font-size:12px"
            onkeydown="if(event.key==='Enter')loadCsv()" title="⚠ ZIP 내 CSV를 직접 수정하면 불러오기가 실패할 수 있습니다">
          <button class="action-btn" onclick="loadCsv()">ZIP 불러오기</button>
          <button class="action-btn primary" onclick="saveCsv()">ZIP 저장</button>
        </div>
        <div class="action-row">
          <span class="info" id="action-info"></span>
          <button class="action-btn" onclick="checkFiles()">파일 존재 확인</button>
          <button class="action-btn danger" onclick="confirmDelete(false)">선택 삭제</button>
          <button class="action-btn danger" onclick="confirmDelete(true)">전체 REMOVE 삭제</button>
        </div>
      </div>
    </div>
  </div>
</div>

<script>
// ── 상태 ──
let state = { regular: [], image: [], video: [], archive: [] };
let currentTab = 'regular';
let filterText = '';
let pollTimer = null;

// ── 페이지 로드 시 서버 상태 복원 ──
(async function restoreState() {
  try {
    const r = await fetch('/api/scan/status');
    const data = await r.json();
    if (data.status === 'scanning') {
      document.getElementById('scan-btn').disabled = true;
      document.getElementById('cancel-btn').style.display = '';
      setStatus('스캔 중...'); setHeaderStatus('스캔 중...');
      pollTimer = setInterval(pollStatus, 800);
    } else if (data.result) {
      state = data.result;
      updateBadges();
      renderGroups();
      setStatus(data.status === 'done' ? '스캔 완료' : data.status);
      setHeaderStatus(data.status === 'done' ? '완료' : data.status);
    }
    if (data.log?.length) updateLog(data.log);
    if (data.paths?.length) {
      document.getElementById('paths-input').value = data.paths.join('\n');
    }
  } catch(e) {}
})();

// ── 스캔 ──
async function startScan() {
  const raw = document.getElementById('paths-input').value.trim();
  if (!raw) { alert('경로를 입력하세요'); return; }
  const paths = raw.split('\n').map(s => s.trim()).filter(Boolean);

  const btn = document.getElementById('scan-btn');
  btn.disabled = true;
  document.getElementById('cancel-btn').style.display = '';
  document.getElementById('log-panel').innerHTML = '';
  setStatus('스캔 중...');
  setHeaderStatus('스캔 중...');

  const iv = id => parseInt(document.getElementById(id)?.value || '0', 10);
  const body = {
    paths,
    no_phash: document.getElementById('opt-no-phash').checked,
    no_vhash: document.getElementById('opt-no-vhash').checked,
    no_archive: document.getElementById('opt-no-archive').checked,
    phash_exact: iv('opt-phash-exact'),
    phash_similar: iv('opt-phash-similar'),
    vhash_exact: iv('opt-vhash-exact'),
    vhash_similar: iv('opt-vhash-similar'),
    vhash_frames: iv('opt-vhash-frames'),
    min_overlap: iv('opt-min-overlap'),
    min_arc_files: iv('opt-min-arc-files'),
  };

  try {
    const r = await fetch('/api/scan', { method: 'POST', headers: {'Content-Type':'application/json'}, body: JSON.stringify(body) });
    if (!r.ok) { const e = await r.json(); alert(e.detail); btn.disabled = false; return; }
    pollTimer = setInterval(pollStatus, 800);
  } catch(e) { alert('요청 실패: ' + e); btn.disabled = false; }
}

async function pollStatus() {
  try {
    const r = await fetch('/api/scan/status');
    const data = await r.json();
    updateLog(data.log);
    if (data.status === 'done' || data.status === 'error' || data.status === 'cancelled') {
      clearInterval(pollTimer);
      document.getElementById('scan-btn').disabled = false;
      document.getElementById('cancel-btn').style.display = 'none';
      if (data.status === 'done' && data.result) {
        state = data.result;
        updateBadges();
        renderGroups();
        setStatus('스캔 완료');
        setHeaderStatus('완료');
      } else if (data.status === 'cancelled') {
        setStatus('중단됨');
        setHeaderStatus('중단');
      } else {
        setStatus('오류 발생');
        setHeaderStatus('오류');
      }
    }
  } catch(e) {}
}

async function cancelScan() {
  const btn = document.getElementById('cancel-btn');
  btn.disabled = true;
  btn.textContent = '중단 중...';
  try {
    await fetch('/api/scan/cancel', { method: 'POST' });
  } catch(e) {}
}

function updateLog(lines) {
  const el = document.getElementById('log-panel');
  el.innerHTML = lines.map(l =>
    `<div class="${l.includes('[오류]') ? 'err' : ''}">${escHtml(l)}</div>`
  ).join('');
  el.scrollTop = el.scrollHeight;
}

// ── 탭 ──
function switchTab(tab) {
  currentTab = tab;
  filterText = '';
  document.getElementById('filter-input').value = '';
  document.querySelectorAll('.tab').forEach(t => t.classList.toggle('active', t.dataset.tab === tab));
  renderGroups();
}

function updateBadges() {
  for (const tab of ['regular','image','video','archive']) {
    document.getElementById('badge-' + tab).textContent = state[tab]?.length || 0;
  }
}

// ── 렌더링 ──
function renderGroups() {
  const groups = filteredGroups();
  const container = document.getElementById('groups-container');

  if (!groups.length) {
    container.innerHTML = '<div class="empty">표시할 그룹이 없습니다</div>';
    updateActionInfo();
    return;
  }

  container.innerHTML = groups.map(g => renderGroup(g)).join('');
  updateActionInfo();
}

function filteredGroups() {
  const groups = state[currentTab] || [];
  if (!filterText) return groups;
  const q = filterText.toLowerCase();
  return groups.filter(g => g.files.some(f => f.path.toLowerCase().includes(q)));
}

function renderGroup(g) {
  const removeCount = g.files.filter(f => !f.keep).length;
  const meta = buildMeta(g);
  const filesHtml = g.files.map((f, fi) => renderFile(g.id, fi, f)).join('');
  return `
    <div class="group" id="group-${g.id}">
      <div class="group-header" onclick="toggleGroup('${g.id}')">
        <span class="group-id">${g.id}</span>
        <span class="group-meta">${meta}</span>
        <span class="group-savable">${g.savable_fmt}</span>
        <span class="group-collapse">▾</span>
      </div>
      <div class="group-body" id="body-${g.id}">${filesHtml}</div>
    </div>`;
}

function buildMeta(g) {
  const n = g.files.length;
  if (currentTab === 'archive') return `${n}개 · 공통 ${g.shared}개`;
  if (currentTab === 'image' || currentTab === 'video') {
    const label = g.category === 'exact' ? '완전동일' : '유사';
    return `${n}개 · ${label}`;
  }
  // 일반: 해시 표시
  const hash = g.files[0]?.hash || '';
  return `${n}개${hash ? ' · ' + hash : ''}`;
}

function renderFile(gid, fi, f) {
  const cls = f.keep === true ? 'keep' : (f.keep === false ? 'remove' : '');
  const parts = f.path.split('/');
  const name = parts.pop();
  const dir = parts.join('/') || '/';
  const extra = currentTab === 'archive' && f.shared ? ` · 공통 ${f.shared}개` : '';
  return `
    <div class="file-row ${cls}" id="file-${gid}-${fi}" tabindex="0" onclick="toggleKeep('${gid}',${fi})" onkeydown="handleFileKey(event,'${gid}',${fi})">
      <div class="file-status"></div>
      <div class="file-info">
        <div class="file-name" title="${escHtml(f.path)}">${escHtml(name)}</div>
        <div class="file-path" title="${escHtml(dir)}">${escHtml(dir)}</div>
      </div>
      <span class="file-size">${f.size_fmt}${extra}</span>
      <button class="finder-btn" onclick="openFinder(event,'${escAttr(f.path)}')">열기</button>
    </div>`;
}

function toggleGroup(gid) {
  const body = document.getElementById('body-' + gid);
  body.style.display = body.style.display === 'none' ? '' : 'none';
}

// ── 키보드 네비게이션 ──
function handleFileKey(e, gid, fi) {
  if (e.key === ' ') {
    e.preventDefault();
    toggleKeep(gid, fi);
  } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    e.preventDefault();
    // 현재 탭의 모든 file-row를 순서대로 수집
    const rows = [...document.querySelectorAll('#groups-container .file-row')];
    const cur = document.getElementById(`file-${gid}-${fi}`);
    const idx = rows.indexOf(cur);
    const next = e.key === 'ArrowDown' ? rows[idx + 1] : rows[idx - 1];
    if (next) { next.focus(); }
  }
}

// ── Keep/Remove 토글 ──
function toggleKeep(gid, fi) {
  const groups = state[currentTab];
  const g = groups.find(x => x.id === gid);
  if (!g) return;
  // 클릭한 파일만 KEEP/REMOVE 토글
  g.files[fi].keep = !g.files[fi].keep;
  const el = document.getElementById(`file-${gid}-${fi}`);
  if (el) el.className = 'file-row ' + (g.files[fi].keep ? 'keep' : 'remove');
  updateActionInfo();
}

// ── 필터 일괄 ──
function applyFilter() {
  filterText = document.getElementById('filter-input').value.trim();
  renderGroups();
}

function clearFilter() {
  filterText = '';
  document.getElementById('filter-input').value = '';
  renderGroups();
}

function bulkAction(keepValue) {
  const q = filterText.toLowerCase();
  const groups = state[currentTab] || [];
  groups.forEach(g => {
    const match = !q || g.files.some(f => f.path.toLowerCase().includes(q));
    if (!match) return;
    if (keepValue === 'keep') {
      g.files.forEach(f => { f.keep = true; });
    } else {
      g.files.forEach(f => { f.keep = false; });
    }
  });
  renderGroups();
}

function bulkKeep() { bulkAction('keep'); }
function bulkRemove() { bulkAction('remove'); }

// ── 파인더 ──
async function openFinder(e, path) {
  e.stopPropagation();
  await fetch('/api/open-finder', { method: 'POST', headers: {'Content-Type':'application/json'}, body: JSON.stringify({path}) });
}

// ── 세션 ZIP 불러오기 ──
async function loadCsv() {
  const path = document.getElementById('load-csv-input').value.trim();
  if (!path) { alert('ZIP 파일 경로를 입력하세요'); return; }
  try {
    const r = await fetch('/api/load-csv', {
      method: 'POST', headers: {'Content-Type':'application/json'},
      body: JSON.stringify({ path })
    });
    if (!r.ok) { const e = await r.json(); alert(e.detail); return; }
    const data = await r.json();
    // 모든 탭 복원
    for (const tab of data.tabs) {
      // 서버 scan_state에서 최신 result 가져오기
    }
    const status = await (await fetch('/api/scan/status')).json();
    if (status.result) {
      state = status.result;
    }
    updateBadges();
    renderGroups();
    const summary = data.tabs.map(t => `${t}:${data.counts[t]}그룹`).join(' / ');
    setStatus(`ZIP 불러오기 완료 — ${summary}`);
    setHeaderStatus('ZIP 로드');
  } catch(e) { alert('불러오기 실패: ' + e); }
}

// ── 세션 ZIP 저장 ──
async function saveCsv() {
  const hasData = Object.values(state).some(g => g.length > 0);
  if (!hasData) { alert('저장할 데이터가 없습니다'); return; }
  const r = await fetch('/api/save-csv', {
    method: 'POST', headers: {'Content-Type':'application/json'},
    body: JSON.stringify({ state })
  });
  if (!r.ok) { const e = await r.json(); alert(e.detail); return; }
  const data = await r.json();
  setStatus(`ZIP 저장 완료: ${data.filename}`);
}

// ── 삭제 ──
function confirmDelete(all) {
  const groups = state[currentTab] || [];
  let targets;
  if (all) {
    targets = groups.flatMap(g => g.files.filter(f => !f.keep).map(f => f.path));
  } else {
    // 필터된 그룹 중 remove 파일
    targets = filteredGroups().flatMap(g => g.files.filter(f => !f.keep).map(f => f.path));
  }
  if (!targets.length) { alert('삭제할 파일이 없습니다'); return; }

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `
    <div class="modal">
      <h2>삭제 확인</h2>
      <p>${targets.length}개 파일을 삭제합니다. 이 작업은 되돌릴 수 없습니다.</p>
      <div style="max-height:120px;overflow-y:auto;font-size:10px;color:var(--text2);margin-bottom:12px">
        ${targets.map(p => `<div>${escHtml(p)}</div>`).join('')}
      </div>
      <div class="modal-btns">
        <button class="action-btn" onclick="this.closest('.modal-overlay').remove()">취소</button>
        <button class="action-btn danger" onclick="doDelete(${JSON.stringify(targets).replace(/</g,'&lt;')}, this)">삭제</button>
      </div>
    </div>`;
  document.body.appendChild(overlay);
  // targets를 클로저로 전달
  overlay.querySelector('.action-btn.danger').addEventListener('click', function() {
    overlay.remove();
    executeDelete(targets);
  });
  overlay.querySelector('.action-btn:not(.danger)').addEventListener('click', () => overlay.remove());
  // innerHTML onclick 제거
  overlay.querySelectorAll('[onclick]').forEach(el => el.removeAttribute('onclick'));
}

async function executeDelete(targets) {
  setStatus('삭제 중...');
  const r = await fetch('/api/delete', {
    method: 'POST', headers: {'Content-Type':'application/json'},
    body: JSON.stringify({ paths: targets })
  });
  const data = await r.json();
  // 삭제된 파일을 모든 탭에서 제거
  const deletedSet = new Set(data.deleted);
  for (const tab of ['regular', 'image', 'video', 'archive']) {
    state[tab] = (state[tab] || [])
      .map(g => ({ ...g, files: g.files.filter(f => !deletedSet.has(f.path)) }))
      .filter(g => g.files.length > 1);
  }
  updateBadges();
  renderGroups();
  const msg = `삭제 완료: ${data.deleted.length}개` + (data.errors.length ? ` / 오류: ${data.errors.length}개` : '');
  setStatus(msg);
}

// ── 파일 존재 확인 ──
async function checkFiles() {
  setStatus('파일 존재 확인 중...');
  const r = await fetch('/api/check-files', { method: 'POST' });
  if (!r.ok) { const e = await r.json(); alert(e.detail); return; }
  const data = await r.json();
  // 서버 최신 result 반영
  const status = await (await fetch('/api/scan/status')).json();
  if (status.result) { state = status.result; }
  updateBadges();
  renderGroups();
  if (data.count === 0) {
    setStatus('확인 완료 — 모든 파일이 존재합니다');
  } else {
    setStatus(`확인 완료 — ${data.count}개 없는 파일 제거됨`);
  }
}

// ── 유틸 ──
function updateActionInfo() {
  const groups = state[currentTab] || [];
  const removeCount = groups.flatMap(g => g.files.filter(f => !f.keep)).length;
  const totalGroups = groups.length;
  document.getElementById('action-info').textContent = `${totalGroups}그룹 · REMOVE ${removeCount}개`;
}

function setStatus(msg) { document.getElementById('status-bar').textContent = msg; }
function setHeaderStatus(msg) { document.getElementById('header-status').textContent = msg; }
function escHtml(s) { return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;'); }
function escAttr(s) { return String(s).replace(/'/g,"\\'").replace(/\\/g,'\\\\'); }
</script>
</body>
</html>"""


@app.get("/", response_class=HTMLResponse)
async def root():
    return HTML


# ──────────────────────────────────────────────
# 엔트리포인트
# ──────────────────────────────────────────────

def main():
    global VERBOSE
    parser = argparse.ArgumentParser(description="중복 파일 탐지 웹 UI")
    parser.add_argument("--port", type=int, default=8877, help="포트 (기본: 8877)")
    parser.add_argument("--verbose", action="store_true", help="상세 로그 출력")
    args = parser.parse_args()
    VERBOSE = args.verbose

    url = f"http://127.0.0.1:{args.port}"
    print(f"Dup Web 시작: {url}")

    # 1초 후 브라우저 오픈
    import threading
    def _open():
        import time; time.sleep(1.2)
        webbrowser.open(url)
    threading.Thread(target=_open, daemon=True).start()

    uvicorn.run(app, host="127.0.0.1", port=args.port, log_level="warning" if not VERBOSE else "info")


if __name__ == "__main__":
    main()
