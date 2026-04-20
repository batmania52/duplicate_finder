use axum::{
    Router,
    routing::{get, post},
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use std::net::SocketAddr;

mod state;
mod api;

use state::new_shared_state;
use api::{scan, files, misc, csv};

pub async fn start_server(port: u16) -> anyhow::Result<()> {
    let shared = new_shared_state();

    let static_dir = std::env::current_dir()
        .unwrap_or_default()
        .join("static");

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
        .fallback_service(ServeDir::new(&static_dir))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .with_state(shared);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Dup Server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let port: u16 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(8765);
    start_server(port).await
}
