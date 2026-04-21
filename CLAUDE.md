# utils-project

유틸리티 스크립트 모음

**프로젝트 경로**: `/Users/macbook/projects/utils-project`

---

## 세션 핸드오프

세션 **첫 번째 응답** 시, obsidian CLI로 최신 handoff 파일을 확인한다:

```bash
# 1. 파일 목록 조회 (마지막 파일이 최신)
obsidian files folder="share/project/utils-project/handoff"

# 2. 최신 파일 읽기
obsidian read path="share/project/utils-project/handoff/<파일명>"
```

파일이 있으면 이전 세션 요약을 간략히 알린다.  
`[project/*]` 태스크가 있으면 tasks-inbox와 대조한다 — inbox에 없으면 등록, 있으면 완료 여부 확인 후 알린다.  
파일이 없으면 별도 언급 없이 진행한다.

> 핸드오프 파일 생성: `/session-close` 스킬 실행

---

## 기본 규칙

- 모든 응답과 작업 기록은 **한국어**로 작성한다.
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

---

## 절대 금지 사항

- **비가역 작업**(파일 삭제, DB 초기화, 대량 덮어쓰기 등)은 실행 전 반드시 사용자 확인을 받는다.
- 동일 오류가 **5회 이상** 반복되면 독자적으로 계속 시도하지 않고 중단 후 보고한다.
- 추측으로 코드를 수정하지 않는다. 확인 → 수정 순서를 지킨다.

---

## Obsidian CLI

Obsidian이 실행 중이면 CLI를 우선 사용한다. 닫혀 있으면 폴백:

| 상황 | 방법 |
|------|------|
| Obsidian 실행 중 | `obsidian read/write/append/tasks` |
| Obsidian 닫혀 있음 | Read / Edit / Write 툴로 직접 파일 접근 |

### tasks-inbox 태스크 등록

태스크 추가 시 prefix는 **반드시 프로젝트 디렉토리명**을 사용한다. 스크립트·컴포넌트명 사용 금지.

```bash
obsidian append path="tasks-inbox.md" content="- [ ] [project/utils-project] 할 일 내용 📅 YYYY-MM-DD"
```

---

## 세션 시작 순서

1. 프로젝트 루트 `CLAUDE.md` 읽기
2. 최신 handoff 파일 확인 및 브리핑
3. 필요 시 `wiki_search`, `recall`로 맥락 보완
4. 실제 파일 구조 확인 후 작업 시작

---

## 프로젝트 구조

```
utils-project/
├── duplicate_finder.py        # 메인 스크립트 — 중복/유사 파일 탐지 CLI (레거시)
├── dup_web.py                 # FastAPI 웹 UI 서버 (레거시, Python)
├── CLAUDE.md                  # 이 파일
├── AGENTS.md                  # Codex 환경용 에이전트 지침
├── BACKLOG.md                 # 기능 백로그
├── results.md                 # 성능 측정 결과 기록
├── .pdca-state.json           # 현재 PDCA 사이클 상태
├── MANUAL.md                  # 일반 사용자 한국어 매뉴얼
├── static/
│   ├── app.js                 # 웹 UI 진입점 (rust-embed로 바이너리 내장)
│   ├── scan-path.js           # 경로 행 UI + 폴더 다이얼로그
│   ├── progress.js            # 진행 카운터 + 게이지 업데이트
│   ├── stats.js               # 타입별 통계 패널 + SVG 도넛 차트
│   ├── diff-view.js           # 이미지/영상 비교 모달
│   ├── preset.js              # 프리셋 저장/불러오기
│   ├── style.css              # 웹 UI 스타일 (라이트/다크 테마)
│   └── delete-log/            # 삭제 이력 JSON (YYYY-MM-DD.json)
├── src-tauri/                 # Tauri + Rust 앱 (메인)
│   ├── Cargo.toml             # workspace 정의
│   ├── tauri.conf.json        # macOS 빌드 설정
│   ├── tauri.win.conf.json    # Windows 빌드 설정 (dlls/ 번들 포함)
│   ├── src/main.rs            # Tauri 진입점 (자동 포트 선택, 서버 ready 신호)
│   ├── crates/
│   │   ├── dup-scanner/       # 스캔 로직 lib crate
│   │   └── dup-server/        # Axum API 서버 bin+lib crate
│   └── .cargo/config.toml    # FFMPEG_DIR 환경변수 (macOS)
├── docs/
│   ├── 01-plan/features/      # Plan 문서
│   ├── 02-design/features/    # Design 문서
│   ├── 02-do/features/        # Do 문서
│   ├── 03-analysis/           # Gap 분석 문서
│   ├── 04-report/             # 완료 보고서
│   └── archive/YYYY-MM/       # 완료된 PDCA 사이클 아카이브
└── *.csv                      # 스캔 결과 CSV (gitignore 대상)
```

### Tauri 앱 빌드

| OS | 명령어 |
|----|--------|
| macOS | `cd src-tauri && cargo tauri build --target aarch64-apple-darwin --config tauri.mac.conf.json` |
| Windows | `cd src-tauri && $env:RC="C:/Program Files (x86)/Windows Kits/10/bin/10.0.26100.0/x64/rc.exe"` → `cargo tauri build --target x86_64-pc-windows-msvc --config tauri.win.conf.json` |

**Windows 주의사항**:
- `src-tauri/dlls/` 폴더에 ffmpeg DLL 7개 필요 (gitignore됨, 로컬에서 직접 복사)
- `.cargo/config.toml`에 `FFMPEG_DIR` 경로 설정 필요 (로컬 전용, gitignore됨)

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

---

## PR 알림 규칙

share/PROTOCOL.md를 참조한다.

---

## 프로젝트 설정 (선택)

bkit 등 특정 플러그인이 불필요한 프로젝트라면 `.claude/settings.json`으로 비활성화:

```json
{
  "plugins": {
    "bkit": { "enabled": false }
  }
}
```
