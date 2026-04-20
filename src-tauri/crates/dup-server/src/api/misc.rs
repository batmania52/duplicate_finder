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
