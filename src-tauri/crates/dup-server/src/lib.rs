pub mod state;
pub mod api;

use axum::{
    Router,
    routing::{get, post},
    response::IntoResponse,
    http::{StatusCode, header},
    extract::Path,
};
use tower_http::cors::{Any, CorsLayer};
use rust_embed::RustEmbed;
use std::net::SocketAddr;

use state::new_shared_state;
use api::{scan, files, misc, csv};

#[derive(RustEmbed)]
#[folder = "../../../static/"]
struct Static;

async fn static_handler(Path(path): Path<String>) -> impl IntoResponse {
    serve_static(&path)
}

async fn index_handler() -> impl IntoResponse {
    serve_static("index.html")
}

fn serve_static(path: &str) -> impl IntoResponse {
    match Static::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime.as_ref().to_string())],
                content.data.into_owned(),
            ).into_response()
        }
        None => (StatusCode::NOT_FOUND, [(header::CONTENT_TYPE, "text/plain".to_string())], b"404 Not Found".to_vec()).into_response(),
    }
}

pub async fn start_server(port: u16, ready_tx: Option<std::sync::mpsc::Sender<()>>) -> anyhow::Result<()> {
    let shared = new_shared_state();

    let app = Router::new()
        .route("/api/scan", post(scan::api_scan))
        .route("/api/scan/status", get(scan::api_scan_status))
        .route("/api/scan/cancel", post(scan::api_scan_cancel))
        .route("/api/platform", get(misc::api_platform))
        .route("/api/open-finder", post(misc::api_open_finder))
        .route("/api/check-files", post(files::api_check_files))
        .route("/api/reset", post(misc::api_reset))
        .route("/api/delete", post(files::api_delete))
        .route("/api/save-csv", post(csv::api_save_csv))
        .route("/api/load-csv", post(csv::api_load_csv))
        .route("/", get(index_handler))
        .route("/{*path}", get(static_handler))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .with_state(shared);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("Dup Server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    // 바인딩 완료 후 ready 신호 전송
    if let Some(tx) = ready_tx {
        let _ = tx.send(());
    }

    axum::serve(listener, app).await?;
    Ok(())
}
