# Contract: Folder Module API (Rust)

Module `crate::folder`. Owns all folder operations; delegates per-photo work to `crate::photo`.
Fallible calls return `Result<_, String>`, never panic; errors are logged at the call site.

## Types

```rust
pub struct FileNode { /* name, path, is_dir, children, thumb_low, thumb_high */ }  // moved here

pub struct FolderState {                 // Tauri-managed (Mutex)
    current: Option<PathBuf>,
    watcher: Option<notify::RecommendedWatcher>,
    cancel:  Option<Arc<AtomicBool>>,
}
```

## Functions

| Function | Signature | Behavior |
|----------|-----------|----------|
| open | `async open(app, state, path) -> Result<FileNode, String>` | cancel previous run; scan; start watcher; start pipeline; return tree |
| scan | `scan_dir(path) -> io::Result<FileNode>` | build tree, exclude `_thumbnail`, fill deterministic thumb paths (no generation) |
| enumerate | `photo_paths(&FileNode) -> Vec<String>` | all supported photos in the tree |
| pipeline start | `start_pipeline(app, root, cancel)` | producer (visible-first) + bounded `std::thread` pool over an `mpsc` channel; each worker calls `photo::ensure_thumbnails` and emits `thumbnail-ready` |
| cancel | (internal) | flip the previous run's `Arc<AtomicBool>` |
| watch | `start_watching(app, path, state)` | `notify` watcher → `folder-changed` (moved from filetree) |

## Supporting change in `photo`

```rust
// photo/thumbnail.rs
pub fn thumbnail_paths(source: &Path) -> Thumbnails;   // deterministic paths, NO file I/O
```

`ensure_thumbnails` is refactored to reuse `thumbnail_paths`. The scanner uses it to populate
`FileNode.thumb_low/thumb_high` without generating.

## Guarantees (map to FR / SC)

- Single owner of folder operations; per-photo work delegated to `photo` (FR-001, FR-006).
- Scan returns the structure without generating thumbnails; `_thumbnail` excluded (FR-002, FR-010, SC-001, SC-006).
- Producer–consumer over a bounded std-thread pool; no new dependency (FR-003, FR-004, SC-002).
- Visible level first, then subfolders (FR-016, SC-008).
- Per-photo `thumbnail-ready` events; deterministic paths known up front (FR-013, FR-015, SC-008).
- Reuse valid thumbnails; only missing generated (FR-005, SC-004).
- Folder switch cancels the previous run (FR-012, SC-005).
- Unreadable entries skipped + logged; one failure never aborts the run (FR-011).
- Identical across OSes (FR-014, SC-007).
