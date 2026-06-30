//! Folder change watcher (moved from `filetree.rs`): emits `folder-changed` so the
//! frontend re-scans. Events inside the `_thumbnail` cache are ignored so the pipeline's
//! own writes do not trigger rescan/cancel/restart churn. The watcher handle lives in `FolderState`.

use std::path::Path;

use log::{error, info, warn};
use notify::{RecursiveMode, Watcher};

use super::FolderState;

const THUMB_DIR: &str = "_thumbnail";

/// True if every affected path lies inside a `_thumbnail` directory (i.e. our own output).
fn all_thumbnail_paths(paths: &[std::path::PathBuf]) -> bool {
    !paths.is_empty()
        && paths
            .iter()
            .all(|p| p.components().any(|c| c.as_os_str() == THUMB_DIR))
}

pub(crate) fn start_watching(app: &tauri::AppHandle, path: &Path, state: &FolderState) {
    let app_clone = app.clone();
    let watch_path = path.to_string_lossy().to_string();

    match notify::recommended_watcher(move |res: notify::Result<notify::Event>| match res {
        Ok(event) => {
            // Ignore the pipeline's own writes into `_thumbnail` to avoid self-triggered churn.
            if all_thumbnail_paths(&event.paths) {
                return;
            }
            use tauri::Emitter;
            app_clone
                .emit(
                    crate::events::FOLDER_CHANGED,
                    crate::events::FolderChanged { path: watch_path.clone() },
                )
                .ok();
        }
        Err(e) => warn!("watch event error for {watch_path}: {e}"),
    }) {
        Ok(mut watcher) => {
            if let Err(e) = watcher.watch(path, RecursiveMode::Recursive) {
                error!("Failed to watch folder: {e}");
                // Drop the stale watcher so we are not left watching a no-longer-open folder.
                *state.watcher.lock().unwrap_or_else(|e| e.into_inner()) = None;
            } else {
                info!("Watching: {}", path.display());
                // Replacing the previous watcher implicitly drops it, stopping the old watch.
                *state.watcher.lock().unwrap_or_else(|e| e.into_inner()) = Some(watcher);
            }
        }
        Err(e) => error!("Failed to create watcher: {e}"),
    }
}
