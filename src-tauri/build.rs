fn main() {
    // MinGW (GNU) 타겟에서는 tauri_build가 windres로 리소스 컴파일을 시도하다 실패함
    // GNU 타겟일 때는 리소스 임베딩 없이 빌드
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") {
        println!("cargo:rerun-if-changed=build.rs");
        return;
    }
    tauri_build::build()
}
