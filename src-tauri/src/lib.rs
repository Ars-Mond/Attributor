mod events;
mod filetree;
mod keywords;
mod photo;
mod types;

// Re-exports required by integration tests (tests/metadata_test.rs)
pub use types::ReadResult;

pub mod photo_metadata {
    pub use super::photo::{
        ensure_thumbnails, read_metadata, write_metadata, Metadata, Photo, Thumbnails,
    };
}

use filetree::{FileNode, WatcherState};
use log::{error, info};
use std::path::Path;
use std::sync::Mutex;
use tauri_plugin_prevent_default::Flags;
use types::SaveRequest;

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
    let filepath = metadata.filepath.clone();
    let filename = metadata.filename.clone();
    let orig_path = Path::new(&filepath);
    info!("save_metadata: {}", orig_path.display());

    let meta = photo::Metadata {
        title: metadata.title,
        description: metadata.description,
        keywords: metadata.keywords,
        category: metadata.categories,
    };

    // ── Determine target path (rename if the stem changed) ──
    let orig_stem = orig_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let new_stem = {
        let s = filename.trim();
        Path::new(s).file_stem().and_then(|s| s.to_str()).unwrap_or(s).to_string()
    };

    let final_path = if !new_stem.is_empty() && new_stem != orig_stem {
        let ext = orig_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let new_name = if ext.is_empty() {
            new_stem.clone()
        } else {
            format!("{}.{}", new_stem, ext)
        };
        orig_path.parent().unwrap_or(orig_path).join(&new_name)
    } else {
        orig_path.to_path_buf()
    };

    if final_path != orig_path {
        // Atomically create the new file (O_CREAT|O_EXCL), copy original bytes,
        // splice metadata into the copy, then delete the original.
        use std::io::Write as _;
        {
            let mut src = std::fs::File::open(orig_path).map_err(|e| e.to_string())?;
            let mut dst = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&final_path)
                .map_err(|e| {
                    let msg = if e.kind() == std::io::ErrorKind::AlreadyExists {
                        format!(
                            "File already exists: {}",
                            final_path.file_name().unwrap_or_default().to_string_lossy()
                        )
                    } else {
                        e.to_string()
                    };
                    error!("Failed to create {}: {msg}", final_path.display());
                    msg
                })?;
            std::io::copy(&mut src, &mut dst).map_err(|e| e.to_string())?;
            dst.flush().map_err(|e| e.to_string())?;
        }

        photo::write_metadata(final_path.to_string_lossy().to_string(), meta)?;

        if let Err(e) = std::fs::remove_file(orig_path) {
            error!("Failed to delete original {}: {e}", orig_path.display());
        }
        info!("renamed: {} → {}", orig_path.display(), final_path.display());
    } else {
        photo::write_metadata(filepath.clone(), meta)?;
    }

    info!("save_metadata: done → {}", final_path.display());
    Ok(final_path.to_string_lossy().to_string())
}

/// Viewer fallback: ensure both thumbnails for a single photo and return their paths.
/// (Folder scans already populate `FileNode.thumb_low`/`thumb_high`; this serves files
/// opened outside a scan.) CPU work runs off the UI thread.
#[tauri::command]
async fn get_thumbnails(path: String) -> Result<photo::Thumbnails, String> {
    tokio::task::spawn_blocking(move || photo::ensure_thumbnails(std::path::Path::new(&path)))
        .await
        .map_err(|e| e.to_string())?
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
            get_thumbnails,
            open_folder,
            open_folder_path,
            scan_folder,
            search_keywords,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
