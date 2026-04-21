// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use std::net::TcpListener;

fn pick_free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("failed to bind port")
        .local_addr()
        .unwrap()
        .port()
}

fn main() {
    tracing_subscriber::fmt::init();

    let port = pick_free_port();

    let (tx, rx) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.block_on(async {
            match dup_server::start_server(port, Some(tx)).await {
                Ok(_) => {}
                Err(e) => tracing::error!("Server error: {}", e),
            }
        });
    });

    let _ = rx.recv_timeout(std::time::Duration::from_secs(5));

    let url = format!("http://127.0.0.1:{}/", port);
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            WebviewWindowBuilder::new(
                app,
                "main",
                WebviewUrl::External(url.parse().unwrap()),
            )
            .title("Dup Finder")
            .inner_size(1280.0, 800.0)
            .resizable(true)
            .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error running tauri app");
}
