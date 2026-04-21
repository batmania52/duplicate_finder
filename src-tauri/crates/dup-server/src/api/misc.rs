use axum::{extract::{State, Extension, Query}, http::{StatusCode, header, HeaderMap}, Json};
use serde::Deserialize;
use crate::state::SharedState;

pub async fn api_platform(Extension(port): Extension<u16>) -> Json<serde_json::Value> {
    let platform = if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "win32"
    } else {
        "linux"
    };
    Json(serde_json::json!({"platform": platform, "port": port}))
}

#[derive(Deserialize)]
pub struct OpenFinderRequest {
    pub path: Option<String>,
}

pub async fn api_open_finder(
    Json(body): Json<OpenFinderRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let path = body.path.filter(|p| !p.is_empty())
        .ok_or_else(|| (StatusCode::BAD_REQUEST, Json(serde_json::json!({"detail": "path 필요"}))))?;

    // 아카이브 내부 경로(zip::내부파일) → zip 파일 경로만
    let real_path = path.split("::").next().unwrap_or(&path).to_string();

    #[cfg(target_os = "macos")]
    std::process::Command::new("open").args(["-R", &real_path]).spawn().ok();

    #[cfg(target_os = "windows")]
    std::process::Command::new("explorer").arg(format!("/select,{}", real_path)).spawn().ok();

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let parent = std::path::Path::new(&real_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or(real_path.clone());
        std::process::Command::new("xdg-open").arg(&parent).spawn().ok();
    }

    Ok(Json(serde_json::json!({"ok": true})))
}

pub async fn api_reset(
    State(state): State<SharedState>,
) -> Json<serde_json::Value> {
    let mut st = state.lock().await;
    st.reset();
    Json(serde_json::json!({"ok": true}))
}

#[derive(Deserialize)]
pub struct FileQuery {
    pub path: String,
}

pub async fn api_pick_folder() -> Json<serde_json::Value> {
    let result = tokio::task::spawn_blocking(|| pick_folder_blocking()).await;
    let path = result.ok().flatten();
    Json(serde_json::json!({ "path": path }))
}

fn pick_folder_blocking() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("osascript")
            .args(["-e", "POSIX path of (choose folder with prompt \"폴더 선택\")"])
            .output()
            .ok()?;
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout)
                .trim()
                .trim_end_matches('/')
                .to_string();
            if !path.is_empty() { return Some(path); }
        }
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "[System.Reflection.Assembly]::LoadWithPartialName('System.windows.forms') | Out-Null; \
                 $f = New-Object System.Windows.Forms.FolderBrowserDialog; \
                 $f.ShowDialog() | Out-Null; $f.SelectedPath"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() { return Some(path); }
        }
    }
    None
}

#[derive(Deserialize)]
pub struct SavePresetRequest {
    pub data: serde_json::Value,
}

pub async fn api_save_preset(
    Json(body): Json<SavePresetRequest>,
) -> Json<serde_json::Value> {
    let result = tokio::task::spawn_blocking(move || {
        pick_save_path_blocking().map(|path| {
            std::fs::write(&path, body.data.to_string()).ok()?;
            Some(path)
        }).flatten()
    }).await;
    match result.ok().flatten() {
        Some(path) => Json(serde_json::json!({ "ok": true, "path": path })),
        None => Json(serde_json::json!({ "ok": false })),
    }
}

pub async fn api_load_preset() -> Json<serde_json::Value> {
    let result = tokio::task::spawn_blocking(|| {
        pick_open_path_blocking().and_then(|path| {
            let content = std::fs::read_to_string(&path).ok()?;
            let data: serde_json::Value = serde_json::from_str(&content).ok()?;
            Some(data)
        })
    }).await;
    match result.ok().flatten() {
        Some(data) => Json(serde_json::json!({ "ok": true, "data": data })),
        None => Json(serde_json::json!({ "ok": false })),
    }
}

fn pick_save_path_blocking() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("osascript")
            .args(["-e", "POSIX path of (choose file name with prompt \"프리셋 저장\" default name \"presets.json\")"])
            .output().ok()?;
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() { return Some(path); }
        }
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "[System.Reflection.Assembly]::LoadWithPartialName('System.windows.forms') | Out-Null; \
                 $f = New-Object System.Windows.Forms.SaveFileDialog; \
                 $f.FileName = 'presets.json'; \
                 $f.Filter = 'JSON|*.json|All|*.*'; \
                 $f.ShowDialog() | Out-Null; $f.FileName"])
            .creation_flags(CREATE_NO_WINDOW)
            .output().ok()?;
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() { return Some(path); }
        }
    }
    None
}

pub async fn api_pick_save_zip() -> Json<serde_json::Value> {
    let result = tokio::task::spawn_blocking(|| pick_save_zip_blocking()).await;
    let path = result.ok().flatten();
    Json(serde_json::json!({ "path": path }))
}

pub async fn api_pick_open_zip() -> Json<serde_json::Value> {
    let result = tokio::task::spawn_blocking(|| pick_open_zip_blocking()).await;
    let path = result.ok().flatten();
    Json(serde_json::json!({ "path": path }))
}

fn pick_save_zip_blocking() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("osascript")
            .args(["-e", "POSIX path of (choose file name with prompt \"ZIP 저장\" default name \"dup_session.zip\")"])
            .output().ok()?;
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() { return Some(path); }
        }
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "[System.Reflection.Assembly]::LoadWithPartialName('System.windows.forms') | Out-Null; \
                 $f = New-Object System.Windows.Forms.SaveFileDialog; \
                 $f.FileName = 'dup_session.zip'; \
                 $f.Filter = 'ZIP|*.zip|All|*.*'; \
                 $f.ShowDialog() | Out-Null; $f.FileName"])
            .creation_flags(CREATE_NO_WINDOW)
            .output().ok()?;
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() { return Some(path); }
        }
    }
    None
}

fn pick_open_zip_blocking() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("osascript")
            .args(["-e", "POSIX path of (choose file with prompt \"ZIP 불러오기\" of type {\"public.zip-archive\", \"com.pkware.zip-archive\"})"])
            .output().ok()?;
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() { return Some(path); }
        }
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "[System.Reflection.Assembly]::LoadWithPartialName('System.windows.forms') | Out-Null; \
                 $f = New-Object System.Windows.Forms.OpenFileDialog; \
                 $f.Filter = 'ZIP|*.zip|All|*.*'; \
                 $f.ShowDialog() | Out-Null; $f.FileName"])
            .creation_flags(CREATE_NO_WINDOW)
            .output().ok()?;
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() { return Some(path); }
        }
    }
    None
}

fn pick_open_path_blocking() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("osascript")
            .args(["-e", "POSIX path of (choose file with prompt \"프리셋 불러오기\" of type {\"public.json\"})"])
            .output().ok()?;
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() { return Some(path); }
        }
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "[System.Reflection.Assembly]::LoadWithPartialName('System.windows.forms') | Out-Null; \
                 $f = New-Object System.Windows.Forms.OpenFileDialog; \
                 $f.Filter = 'JSON|*.json|All|*.*'; \
                 $f.ShowDialog() | Out-Null; $f.FileName"])
            .creation_flags(CREATE_NO_WINDOW)
            .output().ok()?;
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() { return Some(path); }
        }
    }
    None
}

fn is_video_ext(path: &std::path::Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
        Some("mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "ts" | "m2ts" | "mpeg" | "mpg")
    )
}

/// ffprobe로 실제 컨테이너 포맷 감지 — WKWebView가 재생 불가능한 포맷 판단
fn needs_remux(path: &std::path::Path) -> bool {
    let Ok(out) = std::process::Command::new("ffprobe")
        .args([
            "-v", "error",
            "-show_entries", "format=format_name",
            "-of", "default=noprint_wrappers=1:nokey=1",
            path.to_str().unwrap_or(""),
        ])
        .output()
    else { return false; };

    let fmt = String::from_utf8_lossy(&out.stdout);
    // mpegts, mpeg, asf 등 브라우저 직접 재생 불가 포맷
    let fmt = fmt.trim();
    fmt.contains("mpegts") || fmt == "mpeg" || fmt.contains("asf")
}

/// MPEG-TS → MP4 remux (ffmpeg -c copy). 캐시 경로 반환.
/// 캐시가 이미 존재하면 그대로 반환.
fn remux_to_mp4(src: &std::path::Path) -> Option<std::path::PathBuf> {
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;

    let mut h = DefaultHasher::new();
    src.hash(&mut h);
    let hash = h.finish();

    let cache_dir = std::env::temp_dir().join("dup-finder-remux");
    std::fs::create_dir_all(&cache_dir).ok()?;
    let out_path = cache_dir.join(format!("{:016x}.mp4", hash));

    if out_path.exists() {
        return Some(out_path);
    }

    let status = std::process::Command::new("ffmpeg")
        .args([
            "-y", "-i", src.to_str()?,
            "-c", "copy",
            "-movflags", "+faststart",
            out_path.to_str()?,
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()?;

    if status.success() { Some(out_path) } else { None }
}

pub async fn api_file(
    Query(q): Query<FileQuery>,
    req_headers: HeaderMap,
) -> axum::response::Response {
    use axum::response::Response;
    use axum::body::Body;
    use tokio::io::{AsyncSeekExt, AsyncReadExt};
    use tokio_util::io::ReaderStream;

    let orig_path = std::path::PathBuf::from(&q.path);

    // 브라우저 재생 불가 컨테이너(mpegts 등)면 MP4로 remux한 캐시 경로 사용
    let serve_path = if is_video_ext(&orig_path) && needs_remux(&orig_path) {
        tokio::task::spawn_blocking({
            let p = orig_path.clone();
            move || remux_to_mp4(&p)
        })
        .await
        .ok()
        .flatten()
        .unwrap_or(orig_path)
    } else {
        orig_path
    };

    let path = serve_path.as_path();
    let file_size = match std::fs::metadata(path) {
        Ok(m) => m.len(),
        Err(_) => return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("not found"))
            .unwrap(),
    };

    let mime = mime_guess::from_path(path).first_or_octet_stream();

    // Range 요청 파싱 (bytes=start-end)
    let range_val = req_headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("bytes="))
        .and_then(|s| {
            let mut parts = s.splitn(2, '-');
            let start: u64 = parts.next()?.parse().ok()?;
            let end: u64 = parts.next()
                .and_then(|e| if e.is_empty() { None } else { e.parse().ok() })
                .unwrap_or(file_size.saturating_sub(1));
            Some((start, end))
        });

    let (start, end, status) = match range_val {
        Some((s, e)) => (s, e.min(file_size.saturating_sub(1)), StatusCode::PARTIAL_CONTENT),
        None => (0, file_size.saturating_sub(1), StatusCode::OK),
    };

    let length = end - start + 1;

    let mut file = match tokio::fs::File::open(path).await {
        Ok(f) => f,
        Err(_) => return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("not found"))
            .unwrap(),
    };

    if start > 0 {
        if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
            return Response::builder()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .body(Body::from("seek error"))
                .unwrap();
        }
    }

    let reader = tokio::io::BufReader::new(file.take(length));
    let stream = ReaderStream::new(reader);

    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, length)
        .header(header::CONTENT_RANGE, format!("bytes {}-{}/{}", start, end, file_size))
        .body(Body::from_stream(stream))
        .unwrap()
}
