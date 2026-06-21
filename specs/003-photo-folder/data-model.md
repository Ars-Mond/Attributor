# Phase 1 Data Model: Photo Folder Handler

## Entities

### Photo Folder (FolderState — managed)

The single owner of the open folder's live resources.

| Field | Type | Notes |
|-------|------|-------|
| `current` | `Option<PathBuf>` | The open folder path |
| `watcher` | `Option<RecommendedWatcher>` | The `notify` watcher (moved from `WatcherState`) |
| `cancel` | `Option<Arc<AtomicBool>>` | Cancel flag for the active generation run |

Held as Tauri-managed state (`Mutex`). Opening a new folder swaps watcher + cancel atomically.

### FileNode (existing; semantics refined)

| Field | Type | Notes |
|-------|------|-------|
| `name`, `path`, `is_dir`, `children` | (unchanged) | tree structure; `_thumbnail` folders excluded |
| `thumb_low` | `Option<String>` | **deterministic** low thumbnail path, set at scan time (file may not exist yet) |
| `thumb_high` | `Option<String>` | deterministic high thumbnail path |

Change vs feature 002: paths are computed at scan (no generation during scan).

### Thumbnail Pipeline (runtime, not serialized)

| Element | Type | Role |
|---------|------|------|
| Producer | enumerates photos needing thumbnails (visible level first, then deeper) | sends jobs |
| Channel | `mpsc::Sender/Receiver<PathBuf>` | job queue |
| Workers | bounded `std::thread` pool (≈ `available_parallelism` − 1, min 1) | call `photo::ensure_thumbnails`, emit `thumbnail-ready` |
| Cancel flag | `Arc<AtomicBool>` | stop on folder switch |

### ThumbnailReady (event payload)

| Field | Type | Notes |
|-------|------|-------|
| `path` | `String` | source photo path whose thumbnail is ready |

`#[derive(Serialize, Clone)]`, `#[serde(rename_all = "camelCase")]` → `{ path }`. Emitted as the
`thumbnail-ready` Tauri event.

### Photo (delegated, existing)

`photo::ensure_thumbnails(&Path)` / `photo::thumbnail::thumbnail_paths(&Path)` /
`photo::read_metadata` — single-photo work the folder module calls but does not reimplement.

## Operations (folder module public surface)

| Operation | Shape | Notes |
|-----------|-------|-------|
| open | `open(app, path) -> FileNode` | scan tree (fast) + start watcher + start pipeline; return tree |
| rescan | `scan(path) -> FileNode` | re-scan after a change; schedule missing thumbnails |
| enumerate photos | `photo_paths(&FileNode) -> Vec<String>` | all supported photos, `_thumbnail` excluded |
| locate thumbnails | via `FileNode.thumb_*` / `photo::thumbnail::thumbnail_paths` | deterministic |
| watch | internal | emits `folder-changed` (existing) |
| cancel | internal | flips the previous run's cancel flag on new open |

## Frontend state

| Element | Type | Notes |
|---------|------|-------|
| `panelState.fileTree` | `FileNode` | existing |
| `panelState.readyThumbs` | reactive `Set<string>` (SvelteSet) | photo paths whose thumbnail is ready |

`FileTree` renders `convertFileSrc(node.thumb_low)` once `readyThumbs.has(node.path)`; placeholder before.

## Lifecycle

```text
open(folder):
  cancel previous run (flip old flag) → swap watcher
  scan_dir → FileNode tree (deterministic thumb paths, _thumbnail excluded)   [fast, returned now]
  spawn producer + bounded worker pool (new cancel flag):
     producer: visible level photos first → deeper subfolders
     worker: ensure_thumbnails(path) (reuse valid) → on Ok emit thumbnail-ready{path}
  return tree
frontend: render tree immediately; on thumbnail-ready{path} → readyThumbs.add(path) → preview shows
```
