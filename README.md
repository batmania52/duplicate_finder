# duplicate_finder

NAS/로컬 디렉토리의 중복 및 유사 파일을 탐지하는 도구.  
CLI와 웹 UI 두 가지 방식으로 사용할 수 있습니다.

## 기능

- **일반 파일 중복**: MD5 해시 기반 완전 일치 탐지
- **이미지 유사도**: pHash 기반 유사 이미지 그룹화 (정확/유사 2단계 임계값)
- **영상 유사도**: 프레임별 pHash 시퀀스 비교
- **압축 파일 내부 검사**: ZIP/TAR 내 파일 해시 비교
- **아카이브 겹침 탐지**: 여러 압축 파일 간 동일 콘텐츠 그룹화 (Union-Find)
- **웹 UI**: 브라우저에서 스캔 → 결과 확인 → 삭제까지 올인원

## 설치

```bash
# 저장소 클론
git clone https://github.com/batmania52/duplicate_finder.git
cd duplicate_finder

# 가상환경 생성 및 활성화
python3 -m venv .venv
source .venv/bin/activate  # Windows: .venv\Scripts\activate

# 의존성 설치
pip install fastapi uvicorn imagehash Pillow cppbktree pybktree

# ffmpeg 설치 (영상 분석에 필요)
brew install ffmpeg  # macOS
# apt install ffmpeg  # Ubuntu
```

> `cppbktree` 설치 실패 시 `pybktree`로 폴백됩니다. 영상 분석은 `ffmpeg`가 없으면 자동으로 건너뜁니다.

## 웹 UI 사용법

```bash
source .venv/bin/activate
python dup_web.py
```

브라우저에서 `http://localhost:8765` 접속

### 스캔

1. 검사할 경로 입력 (여러 경로는 줄바꿈으로 구분)
2. 고급 옵션에서 임계값 조정 (선택)
3. **스캔 시작** 클릭

### 결과 확인

- 탭: **일반 / 이미지 / 영상 / 아카이브**
- 각 파일 행에서 **KEEP / REMOVE** 상태 토글 (클릭 또는 스페이스바)
- 키보드 ↑↓ 로 파일 간 이동
- 경로 필터로 특정 경로 일괄 KEEP/REMOVE

### 저장 및 불러오기

- **ZIP 저장**: 현재 4개 탭 전체 상태를 ZIP 파일로 저장
- **ZIP 불러오기**: 저장된 ZIP 파일을 불러와 작업 재개
  - 같은 스캔 세션(UUID)에서 생성된 ZIP만 로드 가능
  - ZIP 파일을 임의로 수정하면 로드 시 경고 표시

### 삭제

- **파일 존재 확인**: 실제로 존재하지 않는 파일을 리스트에서 제거
- **선택 삭제**: 현재 탭에서 REMOVE 상태인 파일 삭제 (필터 적용 중이면 필터 범위 내)
- **전체 REMOVE 삭제**: 현재 탭 전체에서 REMOVE 상태인 파일 모두 삭제
- 삭제 후 다른 탭에도 자동 반영

### 고급 옵션

| 옵션 | 기본값 | 설명 |
|------|--------|------|
| 이미지 skip | OFF | pHash 계산 건너뛰기 |
| 이미지 정확 임계값 | 0 | 해밍 거리 ≤ 이 값이면 정확 중복 |
| 이미지 유사 임계값 | 10 | 해밍 거리 ≤ 이 값이면 유사 이미지 |
| 영상 skip | OFF | 영상 pHash 계산 건너뛰기 |
| 영상 정확 임계값 | 0 | |
| 영상 유사 임계값 | 10 | |
| 영상 프레임 수 | 10 | 유사도 비교에 사용할 샘플 프레임 수 |
| 아카이브 최소 겹침 | 5 | 겹치는 파일이 이 수 이상이어야 그룹화 |
| 아카이브 최소 파일 수 | 0 | 압축 파일 내 파일 수 필터 (0=제한없음) |

## CLI 사용법

```bash
# 기본 스캔
python duplicate_finder.py scan /Volumes/NAS/photos

# 여러 경로 동시 스캔
python duplicate_finder.py scan /Volumes/NAS/photos /Volumes/NAS/backup

# 이미지 유사도 임계값 조정
python duplicate_finder.py scan /Volumes/NAS/ --phash-exact 3 --phash-similar 15

# 이미지 유사도 검사 끄기
python duplicate_finder.py scan /Volumes/NAS/ --no-phash

# 압축 파일 내부 검사 끄기
python duplicate_finder.py scan /Volumes/NAS/ --no-archive

# 아카이브 겹침 기준 변경
python duplicate_finder.py scan /Volumes/NAS/ --min-overlap 5

# 결과 CSV 파일명 지정
python duplicate_finder.py scan /Volumes/NAS/ -o result.csv

# CSV 기반 삭제 (dry-run)
python duplicate_finder.py delete result.csv --dry-run

# CSV 기반 실제 삭제
python duplicate_finder.py delete result.csv
```

## 프로젝트 구조

```
duplicate_finder/
├── duplicate_finder.py   # 핵심 탐지 로직 및 CLI
├── dup_web.py            # FastAPI 웹 UI 서버
├── results.md            # 성능 측정 기록
└── README.md
```

## 의존성

| 패키지 | 용도 | 필수 여부 |
|--------|------|-----------|
| `fastapi`, `uvicorn` | 웹 UI 서버 | 웹 UI 사용 시 |
| `imagehash`, `Pillow` | 이미지/영상 pHash | 선택 |
| `cppbktree` | BK-tree C++ 구현 (고속 유사도 탐색) | 선택 |
| `pybktree` | cppbktree 폴백 | 선택 |
| `ffmpeg` (시스템) | 영상 프레임 추출 | 선택 |
