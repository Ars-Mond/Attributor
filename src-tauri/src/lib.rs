pub mod batch;
pub mod events;
pub mod folder;
mod keywords;
mod photo;
mod types;

// Re-exports required by integration tests (tests/metadata_test.rs)
pub use types::{ReadResult, SaveRequest};

pub mod photo_metadata {
    pub use super::photo::{
        ensure, ensure_thumbnails, read_metadata, thumbnail_dir_exists, write_metadata, Metadata,
        Photo, Thumbnails,
    };
}

use batch::{cancel_batch, save_metadata_batch, BatchState};
use folder::{FileNode, FolderState, GenConfig, PhotoFolder};
use log::info;
use std::path::Path;
use tauri::Manager;
use tauri_plugin_prevent_default::Flags;

// ── Tauri command mirrors ─────────────────────────────────────────────────

#[tauri::command]
fn search_keywords(query: String, limit: Option<usize>) -> Vec<String> {
    keywords::search_keywords_impl(query, limit)
}

#[tauri::command]
fn read_metadata(path: String) -> Result<ReadResult, String> {
    let meta = photo::read_metadata(path)?;
    Ok(ReadResult {
        title: meta.title,
        description: meta.description,
        keywords: meta.keywords,
        categories: meta.category,
        release_filename: String::new(),
    })
}

#[tauri::command]
fn save_metadata(metadata: SaveRequest) -> Result<String, String> {
    // Single-file save delegates to the same per-file path used by every batch item,
    // so the result is identical whether saved alone or as part of a batch.
    batch::save_one(metadata)
}

/// On-demand single-photo generation: produce the requested size(s) and return both cache paths.
/// Used by the viewer (high) and lazy list previews (low); CPU work runs off the UI thread.
/// An explicit viewer-open uses this regardless of the folder scope (FR-017).
#[tauri::command]
async fn cache_thumbnail(path: String, low: bool, high: bool) -> Result<photo::Thumbnails, String> {
    tokio::task::spawn_blocking(move || photo::ensure(std::path::Path::new(&path), low, high))
        .await
        .map_err(|e| e.to_string())?
}

/// Whether the `_thumbnail` cache folder still exists for an opened folder. The UI calls this when
/// switching to a thumbnail view so a cache deleted on disk can be detected and regenerated.
#[tauri::command]
fn thumbnail_dir_exists(path: String) -> bool {
    photo::thumbnail_dir_exists(Path::new(&path))
}

#[tauri::command]
async fn scan_folder(
    app: tauri::AppHandle,
    state: tauri::State<'_, FolderState>,
    path: String,
    gen: GenConfig,
) -> Result<FileNode, String> {
    PhotoFolder::rescan(&app, state.inner(), Path::new(&path), gen).await
}

#[tauri::command]
async fn open_folder_path(
    app: tauri::AppHandle,
    state: tauri::State<'_, FolderState>,
    path: String,
    gen: GenConfig,
) -> Result<FileNode, String> {
    PhotoFolder::open(&app, state.inner(), Path::new(&path), gen).await
}

#[tauri::command]
async fn open_folder(
    app: tauri::AppHandle,
    state: tauri::State<'_, FolderState>,
    gen: GenConfig,
) -> Result<Option<FileNode>, String> {
    use tauri_plugin_dialog::DialogExt;

    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |result| {
        let _ = tx.send(result);
    });
    let Some(folder) = rx.await.map_err(|e| e.to_string())? else {
        info!("open_folder: cancelled");
        return Ok(None);
    };
    let path = folder.into_path().map_err(|e| e.to_string())?;
    info!("open_folder: {}", path.display());
    let node = PhotoFolder::open(&app, state.inner(), &path, gen).await?;
    Ok(Some(node))
}

// ── App entry ─────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Registered first: a second launch focuses the existing window instead of
        // opening another instance.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .manage(FolderState::default())
        .manage(BatchState::default())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(if cfg!(debug_assertions) {
                    log::LevelFilter::Trace
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
        .plugin(tauri_plugin_prevent_default::Builder::new()
            .with_flags(Flags::all().difference(Flags::RELOAD))
            .build())
        .invoke_handler(tauri::generate_handler![
            read_metadata,
            save_metadata,
            save_metadata_batch,
            cancel_batch,
            cache_thumbnail,
            thumbnail_dir_exists,
            open_folder,
            open_folder_path,
            scan_folder,
            search_keywords,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
