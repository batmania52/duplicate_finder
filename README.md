# Dup Finder

NAS / 로컬 디렉토리의 중복·유사 파일을 탐지하고 정리하는 데스크탑 앱.

Tauri + Rust 백엔드, 웹 UI 프론트엔드로 구성된 크로스플랫폼 앱입니다.

---

## 주요 기능

| 기능 | 설명 |
|------|------|
| **해시 중복 탐지** | SHA-256으로 완전히 동일한 파일 그룹화 |
| **이미지 유사도** | pHash 기반 유사 이미지 탐지 (exact / similar 2단계) |
| **영상 유사도** | 프레임 샘플 해시로 유사 영상 탐지 |
| **아카이브 분석** | ZIP / 7z 내부 파일이 겹치는 압축 파일 그룹화 |
| **KEEP / REMOVE** | 파일별 유지·삭제 표시 후 일괄 실행 |
| **세션 저장** | 작업 결과를 ZIP으로 저장하고 이어서 작업 |
| **프리셋** | 자주 쓰는 경로·옵션 조합 저장 / 불러오기 |
| **진행률 게이지** | 단계별 실시간 진행 상황 표시 |

---

## 다운로드

[Releases](https://github.com/batmania52/duplicate_finder/releases) 페이지에서 최신 버전을 받으세요.

| OS | 파일 |
|----|------|
| macOS (Apple Silicon) | `dup-finder_x.x.x_aarch64.dmg` |
| Windows (x64) | `dup-finder_x.x.x_x64-setup.exe` |

---

## 사용법

자세한 내용은 **[MANUAL.md](MANUAL.md)** 를 참고하세요.

### 빠른 시작

1. 앱 실행
2. 왼쪽 사이드바에서 스캔할 폴더 추가 (📁 아이콘 또는 **+ 경로 추가**)
3. **스캔 시작** 클릭
4. 결과 탭(일반 / 이미지 / 영상 / 아카이브)에서 파일 확인
5. 삭제할 파일을 클릭해 **REMOVE** 표시
6. **전체 REMOVE 삭제** 로 일괄 삭제

---

## 빌드

### 요구사항

- Rust 1.75+
- Node.js (Tauri CLI용)
- ffmpeg (영상 분석)
  - macOS: `brew install ffmpeg`
  - Windows: [ffmpeg 공식 사이트](https://ffmpeg.org/download.html)에서 다운로드 후 `src-tauri/dlls/` 에 DLL 배치

### macOS

```bash
cd src-tauri
cargo tauri build --target aarch64-apple-darwin
```

빌드 결과: `src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/`

### Windows

```powershell
cd src-tauri
$env:RC="C:/Program Files (x86)/Windows Kits/10/bin/10.0.26100.0/x64/rc.exe"
cargo tauri build --target x86_64-pc-windows-msvc --config tauri.win.conf.json
```

> `src-tauri/dlls/` 에 ffmpeg DLL 7개 필요 (gitignore됨, 로컬에서 직접 복사)  
> `.cargo/config.toml` 에 `FFMPEG_DIR` 경로 설정 필요

---

## 프로젝트 구조

```
utils-project/
├── static/               # 웹 UI (HTML / JS / CSS)
│   ├── index.html
│   ├── app.js
│   ├── progress.js       # 진행률 게이지
│   ├── stats.js          # 통계 패널
│   ├── preset.js         # 프리셋 관리
│   ├── diff-view.js      # 이미지·영상 비교
│   └── style.css
├── src-tauri/
│   ├── crates/
│   │   ├── dup-scanner/  # 스캔 로직 (해시·pHash·vHash·아카이브)
│   │   └── dup-server/   # Axum API 서버
│   ├── src/main.rs       # Tauri 진입점
│   └── tauri.conf.json
├── MANUAL.md             # 사용자 매뉴얼
└── BACKLOG.md            # 기능 백로그
```

---

## 레거시

Python 기반 CLI / FastAPI 웹 UI (`duplicate_finder.py`, `dup_web.py`)는 레거시로 유지됩니다.  
현재 메인 개발은 Tauri 앱 기준입니다.
