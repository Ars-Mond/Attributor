//! Concurrent thumbnail generation — a producer–consumer pool over standard threads
//! (no new dependency; constitution v1.1.0 §VIII). The producer enqueues photos
//! visible-level first, then deeper subfolders; a bounded set of workers calls
//! `photo::ensure_thumbnails` and emits a `thumbnail-ready` event per completed photo.
//! A shared cancel flag stops the run when the folder is switched.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

use log::{debug, warn};
use tauri::{AppHandle, Emitter};

use super::{scan, FileNode};
use crate::events::{ThumbnailReady, THUMBNAIL_READY};

/// Start a generation run over `root`. Returns immediately; work happens on worker threads.
pub(crate) fn start(app: AppHandle, root: FileNode, cancel: Arc<AtomicBool>) {
    let jobs = collect_jobs(&root);
    if jobs.is_empty() {
        return;
    }

    let worker_count = thread::available_parallelism()
        .map(|n| n.get().saturating_sub(1).max(1))
        .unwrap_or(1);

    // Pre-fill the queue, then close it so workers exit once it is drained.
    let (tx, rx) = mpsc::channel::<String>();
    for job in jobs {
        let _ = tx.send(job);
    }
    drop(tx);
    let rx = Arc::new(Mutex::new(rx));

    for _ in 0..worker_count {
        let rx = rx.clone();
        let cancel = cancel.clone();
        let app = app.clone();
        thread::spawn(move || {
            loop {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let next = rx.lock().unwrap_or_else(|e| e.into_inner()).recv();
                let path = match next {
                    Ok(p) => p,
                    Err(_) => break, // queue drained
                };
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                match crate::photo::ensure_thumbnails(std::path::Path::new(&path)) {
                    Ok(_) => {
                        if !cancel.load(Ordering::Relaxed) {
                            if let Err(e) =
                                app.emit(THUMBNAIL_READY, ThumbnailReady { path: path.clone() })
                            {
                                warn!("emit thumbnail-ready for {path}: {e}");
                            }
                        }
                    }
                    Err(e) => warn!("thumbnail generation failed for {path}: {e}"),
                }
            }
            debug!("thumbnail worker exiting");
        });
    }
}

/// Photo paths needing thumbnails, ordered visible level first then deeper (breadth-first).
fn collect_jobs(root: &FileNode) -> Vec<String> {
    let mut jobs = Vec::new();
    let mut queue: VecDeque<&FileNode> = VecDeque::new();
    queue.push_back(root);
    while let Some(node) = queue.pop_front() {
        // Photos at this level first.
        for child in &node.children {
            if !child.is_dir && scan::is_supported_image_name(&child.name) {
                jobs.push(child.path.clone());
            }
        }
        // Then descend into subfolders (subsequent levels).
        for child in &node.children {
            if child.is_dir {
                queue.push_back(child);
            }
        }
    }
    jobs
}

#[cfg(test)]
mod tests {
    use super::collect_jobs;
    use crate::folder::FileNode;

    fn file(name: &str) -> FileNode {
        FileNode { name: name.into(), path: name.into(), is_dir: false, children: vec![], thumb_low: None, thumb_high: None }
    }
    fn dir(name: &str, children: Vec<FileNode>) -> FileNode {
        FileNode { name: name.into(), path: name.into(), is_dir: true, children, thumb_low: None, thumb_high: None }
    }

    #[test]
    fn collect_jobs_visible_level_first() {
        let root = dir("root", vec![
            file("top1.jpg"),
            dir("sub", vec![file("deep1.jpg")]),
            file("top2.jpg"),
        ]);
        let jobs = collect_jobs(&root);

        let i_top1 = jobs.iter().position(|p| p == "top1.jpg").expect("top1");
        let i_top2 = jobs.iter().position(|p| p == "top2.jpg").expect("top2");
        let i_deep = jobs.iter().position(|p| p == "deep1.jpg").expect("deep1");
        assert!(i_top1 < i_deep && i_top2 < i_deep, "visible level before deeper: {jobs:?}");
    }
}
