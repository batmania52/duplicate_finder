use axum::{extract::{State, Extension, Query}, http::{StatusCode, header}, response::IntoResponse, Json};
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

pub async fn api_file(
    Query(q): Query<FileQuery>,
) -> impl IntoResponse {
    let path = std::path::Path::new(&q.path);
    match std::fs::read(path) {
        Ok(bytes) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime.as_ref().to_string())],
                bytes,
            ).into_response()
        }
        Err(_) => (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain".to_string())],
            b"not found".to_vec(),
        ).into_response(),
    }
}
