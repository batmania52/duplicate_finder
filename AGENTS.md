# AGENTS.md

유틸리티 스크립트 모음 작업용 프로젝트 루트다.

**프로젝트 경로**: `/Users/macbook/projects/utils-project`

## 최우선 규칙

<critical>
- 세션 시작 직후 이 파일을 읽고, Obsidian Vault에서
  `share/project/utils-project/handoff` 최신 handoff 파일부터 확인한다.
- handoff 확인 전에는 답변, 분석, 수정, 실행 등 실질 작업을 진행하지 않는다.
- handoff 확인 후 핵심 내용을 짧게 브리핑한다.
</critical>

handoff 확인 방법 (Codex 환경):

```bash
# Vault handoff 경로: share/project/utils-project/handoff
ls ~/Documents/Obsidian\ Vault/share/project/utils-project/handoff/ 2>/dev/null
LATEST=$(ls -t ~/Documents/Obsidian\ Vault/share/project/utils-project/handoff/ 2>/dev/null | head -1)
[ -n "$LATEST" ] && cat ~/Documents/Obsidian\ Vault/share/project/utils-project/handoff/$LATEST
```

> Obsidian CLI 사용 가능 시: `obsidian files folder="share/project/utils-project/handoff"`

## 바로 적용할 기준

- 모든 응답과 작업 기록은 한국어로 작성한다.
- 말투는 부드럽고 실무적으로 유지한다.
- 기억에 의존하지 말고 먼저 파일과 구조를 직접 확인한다.
- 확인 우선순위: `ls`/`find` → `cat`/`head`/`grep`
- 이미 이번 세션에서 읽은 파일은 재확인 없이 활용해도 된다.

### Turn 수 관리

`/turn-check`로 현재 상태를 확인하고 아래 기준으로 행동한다:

| 턴 수 | 행동 |
|-------|------|
| ~40턴 | 별도 언급 없음 |
| 40~49턴 | "슬슬 마무리 고려하실 때입니다" 한 줄 언급 |
| 50턴+ | "50턴을 넘었습니다. 새 세션 시작을 권장합니다." 명확히 건의 |

## 절대 금지 사항

- **비가역 작업**(파일 삭제, DB 초기화, 대량 덮어쓰기 등)은 실행 전 반드시 사용자 확인을 받는다.
- 동일 오류가 **5회 이상** 반복되면 독자적으로 계속 시도하지 않고 중단 후 보고한다.
- 추측으로 코드를 수정하지 않는다. 확인 → 수정 순서를 지킨다.

## 세션 시작 순서

1. 현재 위치와 대상 폴더를 확인한다.
2. 루트 `AGENTS.md`를 읽는다.
3. `utils-project` 최신 handoff를 확인하고 브리핑한다.
4. 부족할 때만 `recall`, `wiki_search`로 맥락을 보완한다.
5. 실제 파일 구조와 핵심 파일을 확인한 뒤 수정이나 실행을 시작한다.

## 저장소 구조

```
utils-project/
├── duplicate_finder.py        # 메인 스크립트 — 중복/유사 파일 탐지 CLI
├── CLAUDE.md                  # Claude Code 세션용 지침
├── AGENTS.md                  # 이 파일 (Codex 환경용)
├── results.md                 # 성능 측정 결과 기록
├── .pdca-state.json           # 현재 PDCA 사이클 상태
├── docs/
│   ├── 01-plan/features/      # Plan 문서
│   ├── 02-design/features/    # Design 문서
│   ├── 02-do/features/        # Do 문서
│   ├── 03-analysis/           # Gap 분석 문서
│   ├── 04-report/             # 완료 보고서
│   └── archive/YYYY-MM/       # 완료된 PDCA 사이클 아카이브
└── *.csv                      # 스캔 결과 CSV
```

### duplicate_finder.py 주요 기능

| 커맨드 | 설명 |
|--------|------|
| `scan [경로...]` | 중복 파일 탐지 + pHash/vHash 유사도 분석 |
| `delete [csv]` | scan 결과 CSV 기반 중복 파일 삭제 |

주요 옵션: `--no-phash`, `--no-vhash`, `--no-archive`, `--output`

### 핵심 의존성

| 패키지 | 용도 |
|--------|------|
| `cppbktree` | BK-tree C++ 구현 (이미지 유사도 탐색 고속화) |
| `pybktree` | cppbktree 폴백 (영상용 커스텀 거리함수 지원) |
| `imagehash`, `Pillow` | 이미지 pHash 계산 |
| `ffmpeg` (시스템) | 영상 프레임 추출 |

### 병렬화 구조 (2026-04-19 perf-improvements)

| 헬퍼 함수 | 대상 | Worker 수 |
|-----------|------|-----------|
| `_parallel_hash()` | 부분/전체 해시 | `_WORKERS` (최대 16) |
| `_parallel_phash()` | 이미지 pHash | `_WORKERS` (최대 16) |
| `_parallel_vphash()` | 영상 pHash | `min(_WORKERS, 8)` |
| `collect_archive_entries()` | 압축파일 추출 | `min(_WORKERS, 8)` |

`_WORKERS = min(os.cpu_count() or 4, 16)`
