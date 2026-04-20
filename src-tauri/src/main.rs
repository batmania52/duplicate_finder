#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

fn main() {
    tracing_subscriber::fmt::init();

    // Axum 서버를 백그라운드 스레드로 기동
    std::thread::spawn(|| {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.block_on(async {
            if let Err(e) = dup_server::start_server(8765).await {
                tracing::error!("Server error: {}", e);
            }
        });
    });

    tauri::Builder::default()
        .setup(|app| {
            // 앱 창이 준비되면 localhost:8765로 이동
            let window = app.get_webview_window("main").unwrap();
            window.navigate("http://127.0.0.1:8765/".parse().unwrap());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error running tauri app");
}
