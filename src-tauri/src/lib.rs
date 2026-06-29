pub mod batch;
pub mod events;
pub mod folder;
mod keywords;
mod ollama;
mod photo;
mod store;
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
use tauri_plugin_log::TimezoneStrategy;
use tauri_plugin_prevent_default::Flags;

// Log line layout: `[date][time][LEVEL][target] message` — level (fixed 5-wide) before target.
const LOG_TS_FORMAT: &[time::format_description::FormatItem<'_>] =
    time::macros::format_description!("[[[year]-[month]-[day]][[[hour]:[minute]:[second]]");

/// Current UTC timestamp formatted as `[YYYY-MM-DD][HH:MM:SS]` for a log line.
fn log_timestamp() -> String {
    TimezoneStrategy::UseUtc.get_now().format(&LOG_TS_FORMAT).unwrap_or_default()
}

/// ANSI SGR foreground color per level — applied to the colored stdout target only (the file stays plain).
fn level_ansi(level: log::Level) -> &'static str {
    match level {
        log::Level::Error => "1;31", // bold red
        log::Level::Warn => "33",    // yellow
        log::Level::Info => "32",    // green
        log::Level::Debug => "36",   // cyan
        log::Level::Trace => "90",   // bright black
    }
}

// ── Tauri command mirrors ─────────────────────────────────────────────────

#[tauri::command]
fn search_keywords(query: String, limit: Option<usize>) -> Vec<String> {
    keywords::search_keywords_impl(query, limit)
}

/// Best-effort OS UI language as a BCP-47 tag (e.g. "ru-RU"). Used once on first launch to pick a
/// default interface language. Returns "en" when the OS locale is unavailable; never errors for the
/// merely-absent case and never panics across the IPC boundary.
#[tauri::command]
fn detect_os_locale() -> Result<String, String> {
    match sys_locale::get_locale() {
        Some(tag) => Ok(tag),
        None => {
            log::warn!("detect_os_locale: OS locale unavailable, defaulting to en");
            Ok("en".to_string())
        }
    }
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
fn save_metadata(
    metadata: SaveRequest,
    state: tauri::State<'_, store::DbState>,
) -> Result<String, String> {
    // Single-file save delegates to the same per-file path used by every batch item,
    // so the result is identical whether saved alone or as part of a batch.
    let old_path = metadata.filepath.clone();
    let stored = store::StoredMetadata {
        title: metadata.title.clone(),
        description: metadata.description.clone(),
        keywords: metadata.keywords.clone(),
        categories: metadata.categories.clone(),
        release_filename: metadata.release_filename.clone(),
    };
    let final_path = batch::save_one(metadata)?;
    // After the file write, refresh the store record and mark it synced (FR-016).
    state.sync_after_save(&old_path, &final_path, &stored);
    Ok(final_path)
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
fn thumbnail_dir_exists(path: String, recursive: bool) -> bool {
    photo::thumbnail_dir_exists(Path::new(&path), recursive)
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
        .manage(ollama::OllamaState::default())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(if cfg!(debug_assertions) {
                    log::LevelFilter::Trace
                } else {
                    log::LevelFilter::Info
                })
                // Per-target formatting: neutralize the shared format, then format each target itself —
                // stdout gets ANSI colors, the log file stays plain (no escape codes).
                .clear_format()
                .clear_targets()
                .target(
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout).format(
                        |out, message, record| {
                            let level = record.level();
                            out.finish(format_args!(
                                "{}[\x1b[{}m{:<5}\x1b[0m][{}] {}",
                                log_timestamp(),
                                level_ansi(level),
                                level,
                                record.target(),
                                message
                            ));
                        },
                    ),
                )
                .target(
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("attributor".into()),
                    })
                    .format(|out, message, record| {
                        out.finish(format_args!(
                            "{}[{:<5}][{}] {}",
                            log_timestamp(),
                            record.level(),
                            record.target(),
                            message
                        ));
                    }),
                )
                .build(),
        )
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_prevent_default::Builder::new()
            .with_flags(Flags::all().difference(Flags::RELOAD))
            .build())
        .setup(|app| {
            // Open the intermediate metadata store in the app-data dir; degrades to direct file
            // access on any error so editing always works (FR-021).
            match app.path().app_data_dir() {
                Ok(dir) => {
                    std::fs::create_dir_all(&dir).ok();
                    app.manage(store::DbState::open(&dir.join("metadata.db")));
                }
                Err(e) => {
                    log::error!("app_data_dir unavailable: {e}; metadata store disabled");
                    app.manage(store::DbState::disabled());
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            read_metadata,
            save_metadata,
            save_metadata_batch,
            cancel_batch,
            store::open_metadata,
            store::store_metadata,
            store::revert_to_file,
            cache_thumbnail,
            thumbnail_dir_exists,
            open_folder,
            open_folder_path,
            scan_folder,
            search_keywords,
            detect_os_locale,
            ollama::ollama_status,
            ollama::ollama_list_models,
            ollama::ollama_pull_model,
            ollama::install_ollama,
            ollama::ollama_cancel,
            ollama::attribute_photo,
            ollama::attribute_batch,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
