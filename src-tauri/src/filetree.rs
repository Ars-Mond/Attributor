use log::{error, info, warn};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use std::sync::Mutex;

pub struct WatcherState(pub Mutex<Option<RecommendedWatcher>>);

#[derive(Serialize, Clone)]
pub struct FileNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Vec<FileNode>,
}

/// Re-scan a folder path (called by the frontend after a `folder-changed` event).
pub async fn scan_folder_impl(path: String) -> Result<FileNode, String> {
    let p = std::path::PathBuf::from(path);
    tokio::task::spawn_blocking(move || scan_dir(&p))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Open a folder by path without a dialog (used to restore last folder on startup).
pub async fn open_folder_path_impl(
    app: tauri::AppHandle,
    path: String,
) -> Result<FileNode, String> {
    let path = std::path::PathBuf::from(&path);
    if !path.is_dir() {
        return Err(format!("Not a directory: {}", path.display()));
    }
    info!("open_folder_path: {}", path.display());
    let scan_path = path.clone();
    let node = tokio::task::spawn_blocking(move || scan_dir(&scan_path))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| {
            let msg = e.to_string();
            error!("scan_dir failed: {msg}");
            msg
        })?;
    start_watching(&app, &path);
    Ok(node)
}

/// Open a folder via native dialog, scan it, and start watching for changes.
pub async fn open_folder_impl(app: tauri::AppHandle) -> Result<Option<FileNode>, String> {
    use tauri_plugin_dialog::DialogExt;

    // Non-blocking: pick_folder fires the callback from a native dialog thread.
    // blocking_pick_folder() would stall the main thread → "Not Responding" on Windows.
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
    let scan_path = path.clone();
    let node = tokio::task::spawn_blocking(move || scan_dir(&scan_path))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| {
            let msg = e.to_string();
            error!("scan_dir failed: {msg}");
            msg
        })?;
    start_watching(&app, &path);
    Ok(Some(node))
}

/// Start watching a folder for changes and emit `folder-changed` events.
fn start_watching(app: &tauri::AppHandle, path: &std::path::Path) {
    use tauri::Manager;
    let app_clone = app.clone();
    let watch_path = path.to_string_lossy().to_string();

    match notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if res.is_ok() {
            use tauri::Emitter;
            app_clone.emit("folder-changed", &watch_path).ok();
        }
    }) {
        Ok(mut watcher) => {
            if let Err(e) = watcher.watch(path, RecursiveMode::Recursive) {
                error!("Failed to watch folder: {e}");
            } else {
                info!("Watching: {}", path.display());
                let state = app.state::<WatcherState>();
                // Replacing the previous watcher implicitly drops it, stopping the old watch.
                *state.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(watcher);
            }
        }
        Err(e) => error!("Failed to create watcher: {e}"),
    }
}

fn scan_dir(path: &std::path::Path) -> std::io::Result<FileNode> {
    let name = path
        .file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .to_string();

    let mut children = Vec::new();

    // Cache is_dir from metadata to avoid repeated syscalls per entry.
    let is_dir = path.metadata().map(|m| m.is_dir()).unwrap_or(false);

    if is_dir {
        let mut entries: Vec<_> = std::fs::read_dir(path)?
            .filter_map(|e| match e {
                Ok(entry) => Some(entry),
                Err(err) => {
                    warn!("scan_dir: skipping entry in {}: {err}", path.display());
                    None
                }
            })
            .collect();

        entries.sort_by(|a, b| {
            let a_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let b_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
            b_dir.cmp(&a_dir).then_with(|| a.file_name().cmp(&b.file_name()))
        });

        for entry in entries {
            let child = entry.path();
            let child_is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if child_is_dir || is_supported_image(&child) {
                match scan_dir(&child) {
                    Ok(node) => children.push(node),
                    Err(err) => warn!("scan_dir: skipping {}: {err}", child.display()),
                }
            }
        }
    }

    Ok(FileNode {
        name,
        path: path.to_string_lossy().to_string(),
        is_dir,
        children,
    })
}

fn is_supported_image(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("jpg" | "jpeg" | "png" | "webp")
    )
}
