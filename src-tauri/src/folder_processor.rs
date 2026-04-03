use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use tauri::AppHandle;

use crate::events::{self, AppEvent};
use crate::photo_processor::PhotoProcessor;

// ── Public types ──────────────────────────────────────────────────────────

pub struct FolderEntry {
    pub path: PathBuf,
    /// Ready thumbnail (360px the longest side) or the placeholder image.
    pub thumb_360: PathBuf,
    /// Ready thumbnail (1920px the longest side) or the original image.
    pub thumb_1920: PathBuf,
    /// `true` if thumbnails were already in cache; `false` means placeholder/original is used.
    pub cached: bool,
}

// ── Internal queue ────────────────────────────────────────────────────────

struct WorkQueue {
    items: VecDeque<PathBuf>,
    shutdown: bool,
}

type Shared = Arc<(Mutex<WorkQueue>, Condvar)>;

// ── FolderProcessor ───────────────────────────────────────────────────────

pub struct FolderProcessor {
    processor: Arc<PhotoProcessor>,
    app: AppHandle,
    placeholder_360: PathBuf,
    shared: Shared,
    workers: Vec<thread::JoinHandle<()>>,
}

impl FolderProcessor {
    /// Create a new processor with `num_workers` background threads.
    ///
    /// Recommended default for `num_workers`:
    /// ```rust
    /// std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).min(4)
    /// ```
    /// For HDD systems, prefer `1` to reduce seek-thrashing.
    pub fn new(
        cache_dir: PathBuf,
        placeholder_360: PathBuf,
        app: AppHandle,
        num_workers: usize,
    ) -> Self {
        let processor = Arc::new(PhotoProcessor::new(cache_dir));
        let shared: Shared = Arc::new((
            Mutex::new(WorkQueue { items: VecDeque::new(), shutdown: false }),
            Condvar::new(),
        ));

        let mut workers = Vec::with_capacity(num_workers);
        for _ in 0..num_workers {
            let shared = Arc::clone(&shared);
            let processor = Arc::clone(&processor);
            let app = app.clone();
            let placeholder = placeholder_360.clone();
            let handle = thread::spawn(move || {
                worker_loop(shared, processor, app, placeholder);
            });
            workers.push(handle);
        }

        Self { processor, app, placeholder_360, shared, workers }
    }

    /// Scan `folder` and return entries immediately.
    ///
    /// - Files with a valid cache sidecar → entry with real thumbnail paths.
    /// - Files without cache → entry with `thumb_360 = placeholder`, `thumb_1920 = original`.
    ///
    /// All uncached files are queued for background processing. When each one
    /// finishes, a `ThumbnailReady` event is emitted to the frontend.
    pub fn open_folder(&self, folder: &Path) -> Result<Vec<FolderEntry>, String> {
        let images = scan_images(folder);

        // Replace queue contents with the new folder's uncached files.
        let (lock, cvar) = &*self.shared;
        {
            let mut q = lock.lock().unwrap();
            q.items.clear();
            for path in &images {
                if self.processor.cached_thumbs(path).is_none() {
                    q.items.push_back(path.clone());
                }
            }
            if !q.items.is_empty() {
                cvar.notify_all();
            }
        }

        // Build result entries (reads only the sidecar — no image decoding here).
        let entries = images
            .into_iter()
            .map(|path| {
                if let Some(thumbs) = self.processor.cached_thumbs(&path) {
                    FolderEntry {
                        thumb_360: thumbs.thumb_360,
                        thumb_1920: thumbs.thumb_1920,
                        cached: true,
                        path,
                    }
                } else {
                    FolderEntry {
                        thumb_360: self.placeholder_360.clone(),
                        thumb_1920: path.clone(), // show original until thumbnail is ready
                        cached: false,
                        path,
                    }
                }
            })
            .collect();

        Ok(entries)
    }

    /// Move `path` to the front of the background queue so it is processed next.
    /// If the path is not in the queue (already done or never queued), does nothing.
    pub fn prioritize(&self, path: &Path) {
        let (lock, cvar) = &*self.shared;
        let mut q = lock.lock().unwrap();
        if let Some(pos) = q.items.iter().position(|p| p == path) {
            let item = q.items.remove(pos).unwrap();
            q.items.push_front(item);
            cvar.notify_one();
        }
    }
}

impl Drop for FolderProcessor {
    fn drop(&mut self) {
        // Signal all workers to stop.
        let (lock, cvar) = &*self.shared;
        lock.lock().unwrap().shutdown = true;
        cvar.notify_all();

        // Join workers. We can't move out of `self.workers` directly because
        // `Drop` takes `&mut self`, so we drain the vec.
        for handle in self.workers.drain(..) {
            let _ = handle.join();
        }
    }
}

// ── Worker thread ─────────────────────────────────────────────────────────

fn worker_loop(
    shared: Shared,
    processor: Arc<PhotoProcessor>,
    app: AppHandle,
    _placeholder: PathBuf, // reserved for future error-state thumbnails
) {
    let (lock, cvar) = &*shared;
    loop {
        // Wait for work or shutdown signal.
        let path = {
            let mut q = cvar
                .wait_while(lock.lock().unwrap(), |q| {
                    q.items.is_empty() && !q.shutdown
                })
                .unwrap();

            if q.shutdown && q.items.is_empty() {
                break;
            }
            q.items.pop_front()
        };

        if let Some(path) = path {
            match processor.process(&path) {
                Ok(result) => {
                    events::invoke(
                        &app,
                        AppEvent::ThumbnailReady {
                            path: path.to_string_lossy().into_owned(),
                            thumb_360: result
                                .thumbnails
                                .thumb_360
                                .to_string_lossy()
                                .into_owned(),
                            thumb_1920: result
                                .thumbnails
                                .thumb_1920
                                .to_string_lossy()
                                .into_owned(),
                        },
                    );
                }
                Err(e) => log::warn!("FolderProcessor: failed to process {}: {e}", path.display()),
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

/// Collect supported image files from `folder`, sorted by name (dirs excluded).
fn scan_images(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };

    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_supported_image(p))
        .collect();

    paths.sort_unstable_by(|a, b| {
        a.file_name().cmp(&b.file_name())
    });

    paths
}

fn is_supported_image(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("jpg") | Some("jpeg") | Some("png") | Some("webp")
    )
}
