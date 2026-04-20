use axum::{extract::State, http::StatusCode, Json};
use chrono::Local;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use dup_scanner::model::ScanOptions;
use crate::state::{SharedState, ScanStatus};

#[derive(Debug, Deserialize)]
pub struct ScanRequest {
    pub paths: Vec<String>,
    #[serde(default)]
    pub no_phash: bool,
    #[serde(default)]
    pub no_vhash: bool,
    #[serde(default)]
    pub no_archive: bool,
    #[serde(default = "default_phash_exact")]
    pub phash_exact: u32,
    #[serde(default = "default_phash_similar")]
    pub phash_similar: u32,
    #[serde(default = "default_vhash_frames")]
    pub vhash_frames: u32,
    #[serde(default = "default_vhash_exact")]
    pub vhash_exact: f32,
    #[serde(default = "default_vhash_similar")]
    pub vhash_similar: f32,
    #[serde(default = "default_min_overlap")]
    pub min_overlap: u32,
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
}

fn default_phash_exact() -> u32 { 0 }
fn default_phash_similar() -> u32 { 10 }
fn default_vhash_frames() -> u32 { 10 }
fn default_vhash_exact() -> f32 { 3.0 }
fn default_vhash_similar() -> f32 { 10.0 }
fn default_min_overlap() -> u32 { 2 }

#[derive(Serialize)]
pub struct ScanStartResponse {
    pub status: String,
}

pub async fn api_scan(
    State(state): State<SharedState>,
    Json(req): Json<ScanRequest>,
) -> Result<Json<ScanStartResponse>, (StatusCode, Json<serde_json::Value>)> {
    let mut st = state.lock().await;
    if st.status == ScanStatus::Scanning {
        return Err((StatusCode::CONFLICT, Json(serde_json::json!({"detail": "스캔이 이미 진행 중입니다"}))));
    }
    for p in &req.paths {
        if !std::path::Path::new(p).is_dir() {
            return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({"detail": format!("디렉토리가 존재하지 않아요: {}", p)}))));
        }
    }

    let cancel = CancellationToken::new();
    let session_uuid = Uuid::new_v4().to_string();
    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();

    st.status = ScanStatus::Scanning;
    st.log.clear();
    st.result = None;
    st.timestamp = Some(timestamp.clone());
    st.paths = req.paths.clone();
    st.session_uuid = Some(session_uuid.clone());
    st.cancel_token = Some(cancel.clone());
    drop(st);

    let options = ScanOptions {
        paths: req.paths,
        no_phash: req.no_phash,
        no_vhash: req.no_vhash,
        no_archive: req.no_archive,
        phash_exact: req.phash_exact,
        phash_similar: req.phash_similar,
        vhash_frames: req.vhash_frames,
        vhash_exact: req.vhash_exact,
        vhash_similar: req.vhash_similar,
        min_overlap: req.min_overlap,
        exclude_patterns: req.exclude_patterns,
    };

    let state_clone = state.clone();
    let cancel_clone = cancel.clone();

    tokio::spawn(async move {
        let (log_tx, mut log_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

        // 로그 수집 태스크
        let state_log = state_clone.clone();
        tokio::spawn(async move {
            while let Some(msg) = log_rx.recv().await {
                let mut st = state_log.lock().await;
                st.log.push(msg);
                if st.log.len() > 500 {
                    st.log.remove(0);
                }
            }
        });

        let result = dup_scanner::run_scan(options, log_tx, cancel_clone).await;
        let mut st = state_clone.lock().await;
        match result {
            Ok(scan_result) => {
                if st.status != ScanStatus::Cancelled {
                    st.status = ScanStatus::Done;
                    st.result = Some(scan_result);
                }
            }
            Err(e) => {
                st.status = ScanStatus::Error;
                st.log.push(format!("오류: {}", e));
            }
        }
    });

    Ok(Json(ScanStartResponse { status: "started".to_string() }))
}

pub async fn api_scan_status(
    State(state): State<SharedState>,
) -> Json<serde_json::Value> {
    let st = state.lock().await;
    Json(serde_json::json!({
        "status": st.status,
        "log": st.log.iter().rev().take(50).rev().collect::<Vec<_>>(),
        "result": st.result,
        "timestamp": st.timestamp,
        "paths": st.paths,
        "session_uuid": st.session_uuid,
    }))
}

pub async fn api_scan_cancel(
    State(state): State<SharedState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let mut st = state.lock().await;
    if st.status != ScanStatus::Scanning {
        return Err((StatusCode::CONFLICT, Json(serde_json::json!({"detail": "스캔 중이 아닙니다"}))));
    }
    if let Some(token) = st.cancel_token.take() {
        token.cancel();
    }
    st.status = ScanStatus::Cancelled;
    st.log.push("중단됨".to_string());
    Ok(Json(serde_json::json!({"status": "cancelled"})))
}
