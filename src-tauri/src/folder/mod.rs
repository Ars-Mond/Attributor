//! `PhotoFolder` — the single owner of folder operations: open/scan a folder and its
//! subfolders, enumerate/search photos, locate thumbnails, watch for changes, and drive
//! concurrent thumbnail generation. Per-photo work (decode, metadata, single-photo
//! thumbnails) is delegated to the `photo` module (single responsibility).

mod pipeline;
mod scan;
mod watch;

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use notify::RecommendedWatcher;
use serde::{Deserialize, Serialize};

pub use scan::scan_dir;

/// A node in the folder tree sent to the frontend.
#[derive(Serialize, Clone)]
pub struct FileNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Vec<FileNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumb_low: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumb_high: Option<String>,
}

/// What the frontend asks the backend to generate eagerly on folder open/scan. Derived from the
/// cache settings: `low`/`high` request each size, `recursive` extends generation into subfolders.
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct GenConfig {
    pub low: bool,
    pub high: bool,
    pub recursive: bool,
}

/// Tauri-managed runtime state for the open folder.
#[derive(Default)]
pub struct FolderState {
    pub watcher: Mutex<Option<RecommendedWatcher>>,
    pub cancel: Mutex<Option<Arc<AtomicBool>>>,
}

/// The folder "class": entry point for all folder operations (mirrors `Photo`).
pub struct PhotoFolder;

impl PhotoFolder {
    /// Open & scan a folder: build the tree fast (no thumbnail generation), start the
    /// watcher, and kick off concurrent thumbnail generation — cancelling any previous run.
    pub async fn open(
        app: &tauri::AppHandle,
        state: &FolderState,
        path: &Path,
        gen: GenConfig,
    ) -> Result<FileNode, String> {
        if !path.is_dir() {
            return Err(format!("Not a directory: {}", path.display()));
        }

        // Scan first (cheap, no generation). Cancel the previous run only AFTER a
        // successful scan, so a failed open never leaves the prior folder un-generating.
        let scan_path = path.to_path_buf();
        let node = tokio::task::spawn_blocking(move || scan::scan_dir(&scan_path))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| {
                let msg = e.to_string();
                log::error!("scan failed: {msg}");
                msg
            })?;

        watch::start_watching(app, path, state);
        let cancel = swap_run(state);
        if gen.low || gen.high {
            pipeline::start(app.clone(), node.clone(), cancel, gen.low, gen.high, gen.recursive);
        }
        Ok(node)
    }

    /// Re-scan after a `folder-changed` event; reschedule generation for missing thumbnails.
    pub async fn rescan(
        app: &tauri::AppHandle,
        state: &FolderState,
        path: &Path,
        gen: GenConfig,
    ) -> Result<FileNode, String> {
        let scan_path = path.to_path_buf();
        let node = tokio::task::spawn_blocking(move || scan::scan_dir(&scan_path))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;

        let cancel = swap_run(state);
        if gen.low || gen.high {
            pipeline::start(app.clone(), node.clone(), cancel, gen.low, gen.high, gen.recursive);
        }
        Ok(node)
    }

    /// Enumerate all supported photo paths in a scanned tree (`_thumbnail` already excluded).
    pub fn photo_paths(root: &FileNode) -> Vec<String> {
        let mut out = Vec::new();
        collect_photos(root, &mut out);
        out
    }
}

/// Atomically cancel the previous run and install a fresh cancel flag; returns the new flag.
/// Done under one lock so interleaved opens can never lose a flag (orphaned worker pool).
fn swap_run(state: &FolderState) -> Arc<AtomicBool> {
    let new_flag = Arc::new(AtomicBool::new(false));
    let mut guard = state.cancel.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(old) = guard.take() {
        old.store(true, Ordering::Relaxed);
    }
    *guard = Some(new_flag.clone());
    new_flag
}

fn collect_photos(node: &FileNode, out: &mut Vec<String>) {
    if !node.is_dir && scan::is_supported_image_name(&node.name) {
        out.push(node.path.clone());
    }
    for child in &node.children {
        collect_photos(child, out);
    }
}
