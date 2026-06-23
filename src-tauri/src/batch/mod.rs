//! Batch metadata save: one async command writes a whole selection concurrently with
//! `rayon` (inside `spawn_blocking`), streams per-file results over a `tauri::ipc::Channel`,
//! and is cancellable via a shared flag in managed `BatchState`. The single-file
//! `save_metadata` command delegates to the same `save_one`, so a batch item's result is
//! identical to saving that file alone. Per-photo write internals live in the `photo` module.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use log::{error, info, warn};

use crate::events::{BatchProgress, ItemStatus};
use crate::types::SaveRequest;

/// Tauri-managed state holding the in-flight batch's cancel flag.
#[derive(Default)]
pub struct BatchState {
    pub cancel: Mutex<Option<Arc<AtomicBool>>>,
}

/// Cancel any previous batch and install a fresh flag; returns the new flag.
/// Done under one lock so interleaved batches can never lose a flag.
fn swap_cancel(state: &BatchState) -> Arc<AtomicBool> {
    let new_flag = Arc::new(AtomicBool::new(false));
    let mut guard = state.cancel.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(old) = guard.take() {
        old.store(true, Ordering::Relaxed);
    }
    *guard = Some(new_flag.clone());
    new_flag
}

/// Write one photo's metadata, renaming the file if its stem changed. Shared by the
/// single-file `save_metadata` command and every batch item (guarantees identical results).
pub(crate) fn save_one(item: SaveRequest) -> Result<String, String> {
    let filepath = item.filepath.clone();
    let filename = item.filename.clone();
    let orig_path = Path::new(&filepath);
    info!("save_one: {}", orig_path.display());

    let meta = crate::photo::Metadata {
        title: item.title,
        description: item.description,
        keywords: item.keywords,
        category: item.categories,
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

        crate::photo::write_metadata(final_path.to_string_lossy().to_string(), meta)?;

        if let Err(e) = std::fs::remove_file(orig_path) {
            error!("Failed to delete original {}: {e}", orig_path.display());
        }
        info!("renamed: {} → {}", orig_path.display(), final_path.display());
    } else {
        crate::photo::write_metadata(filepath.clone(), meta)?;
    }

    info!("save_one: done → {}", final_path.display());
    Ok(final_path.to_string_lossy().to_string())
}

/// Pure batch core (no Tauri): save every item concurrently with `rayon`, reporting each
/// outcome through `progress` and returning the outcomes in input order. An item is skipped
/// to `Cancelled` only if it has not started when `cancel` is observed; an in-flight write
/// finishes. Testable without a Tauri runtime.
pub fn save_batch(
    items: Vec<SaveRequest>,
    cancel: &Arc<AtomicBool>,
    progress: impl Fn(BatchProgress) + Send + Sync,
) -> Vec<ItemStatus> {
    use rayon::prelude::*;
    items
        .into_par_iter()
        .enumerate()
        .map(|(index, item)| {
            let status = if cancel.load(Ordering::Relaxed) {
                ItemStatus::Cancelled
            } else {
                match save_one(item) {
                    Ok(path) => ItemStatus::Ok { path },
                    Err(error) => {
                        warn!("batch item {index} failed: {error}");
                        ItemStatus::Failed { error }
                    }
                }
            };
            progress(BatchProgress { index, status: status.clone() });
            status
        })
        .collect()
}

/// Save a batch concurrently, streaming per-file progress over `on_progress` and returning
/// the ordered outcomes. Best-effort: a per-file failure never aborts the batch.
#[tauri::command]
pub async fn save_metadata_batch(
    items: Vec<SaveRequest>,
    on_progress: tauri::ipc::Channel<BatchProgress>,
    state: tauri::State<'_, BatchState>,
) -> Result<Vec<ItemStatus>, String> {
    let cancel = swap_cancel(state.inner());
    tokio::task::spawn_blocking(move || {
        save_batch(items, &cancel, move |msg| {
            if let Err(e) = on_progress.send(msg) {
                warn!("batch progress send failed: {e}");
            }
        })
    })
    .await
    .map_err(|e| e.to_string())
}

/// Request cancellation of the in-flight batch. Not-yet-started items resolve to
/// `Cancelled`; an item already mid-write finishes. Idempotent and safe with no batch running.
#[tauri::command]
pub fn cancel_batch(state: tauri::State<'_, BatchState>) {
    let guard = state.cancel.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(flag) = guard.as_ref() {
        flag.store(true, Ordering::Relaxed);
    }
}
