use axum::{extract::State, http::StatusCode, Json};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::path::Path;
use crate::state::SharedState;

#[derive(Deserialize)]
pub struct DeleteRequest {
    pub paths: Vec<String>,
}

#[derive(Serialize)]
pub struct DeleteResponse {
    pub deleted: Vec<String>,
    pub errors: Vec<serde_json::Value>,
}

pub async fn api_check_files(
    State(state): State<SharedState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let mut st = state.lock().await;
    if st.result.is_none() {
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({"detail": "스캔 결과가 없습니다"}))));
    }

    fn path_exists(path: &str) -> bool {
        let real = path.split("::").next().unwrap_or(path);
        Path::new(real).exists()
    }

    let mut missing: Vec<String> = Vec::new();
    if let Some(result) = &mut st.result {
        for tab_groups in [
            &mut result.regular,
            &mut result.image,
            &mut result.video,
            &mut result.archive,
        ] {
            let mut new_groups = Vec::new();
            for g in tab_groups.iter() {
                let kept: Vec<_> = g.files.iter().filter(|f| path_exists(&f.path)).cloned().collect();
                let gone: Vec<_> = g.files.iter().filter(|f| !path_exists(&f.path))
                    .map(|f| f.path.clone()).collect();
                missing.extend(gone);
                if kept.len() >= 1 {
                    let mut ng = g.clone();
                    ng.files = kept;
                    new_groups.push(ng);
                }
            }
            *tab_groups = new_groups;
        }
    }

    let count = missing.len();
    if count > 0 {
        st.log.push(format!("[파일 확인 완료] 없는 파일 {}개 제거됨", count));
    } else {
        st.log.push("[파일 확인 완료] 전체 정상".to_string());
    }
    Ok(Json(serde_json::json!({"missing": missing, "count": count})))
}

pub async fn api_delete(
    State(state): State<SharedState>,
    Json(req): Json<DeleteRequest>,
) -> Json<DeleteResponse> {
    let mut deleted = Vec::new();
    let mut errors: Vec<serde_json::Value> = Vec::new();
    let mut log_entries: Vec<serde_json::Value> = Vec::new();

    for path in &req.paths {
        if path.contains("::") {
            continue; // zip 내부 항목 건너뜀
        }
        match std::fs::metadata(path) {
            Ok(meta) if meta.is_file() => {
                let size = meta.len();
                match std::fs::remove_file(path) {
                    Ok(_) => {
                        deleted.push(path.clone());
                        log_entries.push(serde_json::json!({
                            "time": Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
                            "path": path,
                            "method": "delete",
                            "size": size,
                        }));
                    }
                    Err(e) => {
                        errors.push(serde_json::json!({"path": path, "error": e.to_string()}));
                    }
                }
            }
            _ => {
                errors.push(serde_json::json!({"path": path, "error": "파일이 존재하지 않음"}));
            }
        }
    }

    append_delete_log(&log_entries);

    // scan_state result에서 삭제된 경로 제거
    if !deleted.is_empty() {
        let deleted_set: std::collections::HashSet<&String> = deleted.iter().collect();
        let mut st = state.lock().await;
        if let Some(result) = &mut st.result {
            for tab_groups in [
                &mut result.regular,
                &mut result.image,
                &mut result.video,
                &mut result.archive,
            ] {
                for g in tab_groups.iter_mut() {
                    g.files.retain(|f| !deleted_set.contains(&f.path));
                }
                tab_groups.retain(|g| g.files.len() > 1);
            }
        }
        st.log.push(format!("[삭제 완료] {}개 성공{}", deleted.len(),
            if errors.is_empty() { String::new() } else { format!(" / {}개 실패", errors.len()) }
        ));
    }

    Json(DeleteResponse { deleted, errors })
}

fn append_delete_log(entries: &[serde_json::Value]) {
    if entries.is_empty() { return; }
    let log_dir = Path::new("static/delete-log");
    let _ = std::fs::create_dir_all(log_dir);
    let log_file = log_dir.join(format!("{}.json", Local::now().format("%Y-%m-%d")));

    let mut existing: Vec<serde_json::Value> = if log_file.exists() {
        std::fs::read_to_string(&log_file)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    existing.extend_from_slice(entries);
    let _ = std::fs::write(&log_file, serde_json::to_string_pretty(&existing).unwrap_or_default());
}
