mod filetree;
mod keywords;
mod types;
mod xmp;

// Re-exports required by integration tests (tests/xmp_read.rs)
pub use types::ReadResult;
pub use xmp::{parse_xmp, read_jpeg_xmp_fast, read_png_xmp_fast, read_webp_xmp_fast};

use filetree::{FileNode, WatcherState};
use std::sync::Mutex;
use types::SaveRequest;

// ── Tauri command mirrors ─────────────────────────────────────────────────

#[tauri::command]
fn search_keywords(query: String, limit: Option<usize>) -> Vec<String> {
    keywords::search_keywords_impl(query, limit)
}

#[tauri::command]
fn read_metadata(path: String) -> Result<ReadResult, String> {
    xmp::read_metadata_impl(path)
}

#[tauri::command]
fn save_metadata(metadata: SaveRequest) -> Result<String, String> {
    xmp::save_metadata_impl(metadata)
}

#[tauri::command]
async fn scan_folder(path: String) -> Result<FileNode, String> {
    filetree::scan_folder_impl(path).await
}

#[tauri::command]
async fn open_folder_path(app: tauri::AppHandle, path: String) -> Result<FileNode, String> {
    filetree::open_folder_path_impl(app, path).await
}

#[tauri::command]
async fn open_folder(app: tauri::AppHandle) -> Result<Option<FileNode>, String> {
    filetree::open_folder_impl(app).await
}

// ── App entry ─────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(WatcherState(Mutex::new(None)))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(if cfg!(debug_assertions) {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Info
                })
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("attributor".into()),
                    },
                ))
                .build(),
        )
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            read_metadata,
            save_metadata,
            open_folder,
            open_folder_path,
            scan_folder,
            search_keywords,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
