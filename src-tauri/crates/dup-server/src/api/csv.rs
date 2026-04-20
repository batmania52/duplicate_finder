use axum::{extract::State, http::StatusCode, Json};
use chrono::Local;
use serde::Deserialize;
use std::io::{Read, Write};
use uuid::Uuid;
use crate::state::SharedState;

#[derive(Deserialize)]
pub struct SaveCsvRequest {
    pub state: serde_json::Value,
    pub path: Option<String>,
}

#[derive(Deserialize)]
pub struct LoadCsvRequest {
    pub path: String,
}

pub async fn api_save_csv(
    State(state): State<SharedState>,
    Json(req): Json<SaveCsvRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let st = state.lock().await;
    let session_uuid = st.session_uuid.clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let timestamp = st.timestamp.clone()
        .unwrap_or_else(|| Local::now().format("%Y%m%d_%H%M%S").to_string());
    drop(st);

    let zip_name = req.path.unwrap_or_else(|| format!("dup_session_{}.zip", timestamp));
    let zip_path = std::path::Path::new(&zip_name);

    let file = std::fs::File::create(zip_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()}))))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let tab_filenames = [
        ("regular", "duplicates.csv"),
        ("image", "image_similar.csv"),
        ("video", "video_similar.csv"),
        ("archive", "archive_overlaps.csv"),
    ];

    let mut manifest_tabs = serde_json::Map::new();

    for (tab, filename) in &tab_filenames {
        let groups_val = req.state.get(*tab).and_then(|v| v.as_array());
        if let Some(groups) = groups_val {
            if groups.is_empty() { continue; }
            let csv_str = groups_to_csv(*tab, groups);
            zip.start_file(*filename, options).ok();
            zip.write_all(csv_str.as_bytes()).ok();
            manifest_tabs.insert(tab.to_string(), serde_json::json!({
                "file": filename,
                "groups": groups.len()
            }));
        }
    }

    let manifest = serde_json::json!({
        "session_uuid": session_uuid,
        "timestamp": timestamp,
        "tabs": manifest_tabs,
    });
    zip.start_file("manifest.json", options).ok();
    zip.write_all(serde_json::to_string_pretty(&manifest).unwrap_or_default().as_bytes()).ok();
    zip.finish().ok();

    Ok(Json(serde_json::json!({"filename": zip_path.to_string_lossy(), "session_uuid": session_uuid})))
}

pub async fn api_load_csv(
    State(state): State<SharedState>,
    Json(req): Json<LoadCsvRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let file = std::fs::File::open(&req.path)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(serde_json::json!({"detail": e.to_string()}))))?;

    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(serde_json::json!({"detail": e.to_string()}))))?;

    // manifest 읽기
    let manifest: serde_json::Value = {
        let mut mf = zip.by_name("manifest.json")
            .map_err(|_| (StatusCode::BAD_REQUEST, Json(serde_json::json!({"detail": "manifest.json 없음"}))))?;
        let mut s = String::new();
        mf.read_to_string(&mut s).ok();
        serde_json::from_str(&s).unwrap_or_default()
    };

    let session_uuid = manifest.get("session_uuid").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let timestamp = manifest.get("timestamp").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let tab_map = [
        ("regular", "duplicates.csv"),
        ("image", "image_similar.csv"),
        ("video", "video_similar.csv"),
        ("archive", "archive_overlaps.csv"),
    ];

    let mut result_json = serde_json::Map::new();
    for (tab, filename) in &tab_map {
        if let Ok(mut entry) = zip.by_name(filename) {
            let mut s = String::new();
            entry.read_to_string(&mut s).ok();
            let groups = csv_to_groups(*tab, &s);
            result_json.insert(tab.to_string(), serde_json::to_value(groups).unwrap_or_default());
        } else {
            result_json.insert(tab.to_string(), serde_json::json!([]));
        }
    }

    let mut st = state.lock().await;
    st.session_uuid = Some(session_uuid.clone());
    st.timestamp = Some(timestamp.clone());

    Ok(Json(serde_json::json!({
        "session_uuid": session_uuid,
        "timestamp": timestamp,
        "result": result_json,
    })))
}

fn fmt_size(bytes: u64) -> String {
    dup_scanner::model::fmt_size(bytes)
}

fn groups_to_csv(tab: &str, groups: &[serde_json::Value]) -> String {
    let mut wtr = csv::Writer::from_writer(Vec::new());
    match tab {
        "regular" => {
            wtr.write_record(&["group_id", "path", "size", "type", "hash", "created", "keep"]).ok();
            for g in groups {
                let gid = g.get("id").and_then(|v| v.as_str()).unwrap_or("");
                if let Some(files) = g.get("files").and_then(|v| v.as_array()) {
                    for f in files {
                        wtr.write_record(&[
                            gid,
                            f.get("path").and_then(|v| v.as_str()).unwrap_or(""),
                            &f.get("size").and_then(|v| v.as_u64()).unwrap_or(0).to_string(),
                            f.get("type").and_then(|v| v.as_str()).unwrap_or("file"),
                            f.get("hash").and_then(|v| v.as_str()).unwrap_or(""),
                            f.get("created").and_then(|v| v.as_str()).unwrap_or(""),
                            if f.get("keep").and_then(|v| v.as_bool()).unwrap_or(false) { "YES" } else { "NO" },
                        ]).ok();
                    }
                    wtr.write_record(&[""; 7]).ok();
                }
            }
        }
        "image" => {
            wtr.write_record(&["category", "group_id", "path", "size_bytes", "type", "phash", "keep"]).ok();
            for g in groups {
                let gid = g.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let cat = g.get("category").and_then(|v| v.as_str()).unwrap_or("");
                if let Some(files) = g.get("files").and_then(|v| v.as_array()) {
                    for f in files {
                        wtr.write_record(&[
                            cat, gid,
                            f.get("path").and_then(|v| v.as_str()).unwrap_or(""),
                            &f.get("size").and_then(|v| v.as_u64()).unwrap_or(0).to_string(),
                            f.get("type").and_then(|v| v.as_str()).unwrap_or("file"),
                            f.get("phash").and_then(|v| v.as_str()).unwrap_or(""),
                            if f.get("keep").and_then(|v| v.as_bool()).unwrap_or(false) { "YES" } else { "NO" },
                        ]).ok();
                    }
                    wtr.write_record(&[""; 7]).ok();
                }
            }
        }
        "archive" => {
            wtr.write_record(&["group_id", "path", "size", "shared_files", "keep"]).ok();
            for g in groups {
                let gid = g.get("id").and_then(|v| v.as_str()).unwrap_or("");
                if let Some(files) = g.get("files").and_then(|v| v.as_array()) {
                    for f in files {
                        wtr.write_record(&[
                            gid,
                            f.get("path").and_then(|v| v.as_str()).unwrap_or(""),
                            &f.get("size").and_then(|v| v.as_u64()).unwrap_or(0).to_string(),
                            &f.get("shared").and_then(|v| v.as_u64()).unwrap_or(0).to_string(),
                            if f.get("keep").and_then(|v| v.as_bool()).unwrap_or(false) { "YES" } else { "NO" },
                        ]).ok();
                    }
                    wtr.write_record(&[""; 5]).ok();
                }
            }
        }
        _ => {}
    }
    String::from_utf8(wtr.into_inner().unwrap_or_default()).unwrap_or_default()
}

fn csv_to_groups(_tab: &str, csv_str: &str) -> Vec<serde_json::Value> {
    let mut rdr = csv::Reader::from_reader(csv_str.as_bytes());
    let mut groups_map: std::collections::HashMap<String, serde_json::Value> = std::collections::HashMap::new();
    let mut order: Vec<String> = Vec::new();

    for record in rdr.deserialize::<std::collections::HashMap<String, String>>().flatten() {
        let gid = record.get("group_id").cloned().unwrap_or_default();
        if gid.is_empty() { continue; }
        if !groups_map.contains_key(&gid) {
            order.push(gid.clone());
            groups_map.insert(gid.clone(), serde_json::json!({
                "id": gid,
                "files": [],
                "savable": 0,
                "savable_fmt": "",
                "category": record.get("category").cloned().unwrap_or_default(),
            }));
        }

        let size: u64 = record.get("size").or(record.get("size_bytes"))
            .and_then(|s| s.parse().ok()).unwrap_or(0);
        let keep = record.get("keep").map(|s| s.to_uppercase() != "NO").unwrap_or(true);

        let mut file = serde_json::json!({
            "path": record.get("path").cloned().unwrap_or_default(),
            "size": size,
            "size_fmt": fmt_size(size),
            "keep": keep,
            "type": record.get("type").cloned().unwrap_or_else(|| "file".to_string()),
        });
        if let Some(hash) = record.get("hash") { file["hash"] = serde_json::json!(hash); }
        if let Some(phash) = record.get("phash") { file["phash"] = serde_json::json!(phash); }
        if let Some(created) = record.get("created") { file["created"] = serde_json::json!(created); }
        if let Some(shared) = record.get("shared_files").and_then(|s| s.parse::<u64>().ok()) {
            file["shared"] = serde_json::json!(shared);
        }

        if let Some(g) = groups_map.get_mut(&gid) {
            g["files"].as_array_mut().unwrap().push(file);
        }
    }

    order.into_iter().filter_map(|gid| {
        let mut g = groups_map.remove(&gid)?;
        let savable: u64 = g["files"].as_array()?.iter().skip(1)
            .map(|f| f["size"].as_u64().unwrap_or(0)).sum();
        g["savable"] = serde_json::json!(savable);
        g["savable_fmt"] = serde_json::json!(fmt_size(savable));
        Some(g)
    }).collect()
}
