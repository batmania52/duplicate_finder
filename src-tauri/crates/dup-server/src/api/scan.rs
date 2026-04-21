use axum::{extract::State, http::StatusCode, Json};
use chrono::Local;
use serde::Serialize;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use dup_scanner::model::ScanOptions;
use crate::state::{SharedState, ScanStatus};

#[derive(Serialize)]
pub struct ScanStartResponse {
    pub status: String,
}

pub async fn api_scan(
    State(state): State<SharedState>,
    Json(options): Json<ScanOptions>,
) -> Result<Json<ScanStartResponse>, (StatusCode, Json<serde_json::Value>)> {
    let mut st = state.lock().await;
    if st.status == ScanStatus::Scanning {
        return Err((StatusCode::CONFLICT, Json(serde_json::json!({"detail": "스캔이 이미 진행 중입니다"}))));
    }
    for p in &options.paths {
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
    st.paths = options.paths.clone();
    st.session_uuid = Some(session_uuid.clone());
    st.cancel_token = Some(cancel.clone());
    drop(st);

    let state_clone = state.clone();
    let cancel_clone = cancel.clone();

    tokio::spawn(async move {
        let (log_tx, mut log_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

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
        "log": st.log,
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
