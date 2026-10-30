fn main() {
    // Only `backend_info` is an app command. Declaring it here makes Tauri generate an
    // `allow-backend-info` permission that capabilities/default.json grants to the main window only.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["backend_info"])),
    )
    .expect("failed to run tauri-build");
}
