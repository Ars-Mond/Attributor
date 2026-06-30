# Phase 0 Research: Photo Folder Handler

Grounded in the current code (`filetree.rs`, `photo/thumbnail.rs`, `lib.rs`, `FilesPanel.svelte`,
`FileTree.svelte`, `filesPanelStore.svelte.ts`) and the 2026-06-21 clarifications.

## 1. Consolidate folder operations into a `folder` module

- **Decision**: Replace `filetree.rs` with a `folder/` module that owns every folder operation
  (open, scan, enumerate/search photos, locate thumbnails, watch, generate). `scan_dir` and the
  `notify` watcher move in unchanged in behavior; the Tauri commands delegate here.
- **Rationale**: The request mandates a single owner of folder operations with single responsibility.
- **Alternatives considered**: keep `filetree.rs` and add a parallel module — rejected (duplication,
  violates Principle V / the request's "single owner").

## 2. Scan builds the tree fast — no inline thumbnail generation

- **Decision**: `scan.rs::scan_dir` builds the `FileNode` tree, excludes `_thumbnail` folders, and
  fills each photo node's `thumb_low`/`thumb_high` with their **deterministic paths** (computed, not
  generated). It performs no decode/generation. Feature 002's inline scan-time generation is removed.
- **Rationale**: Tree-first, thumbnails-async (clarification). The scan returns immediately.
- **Supporting change**: add `pub fn photo::thumbnail::thumbnail_paths(&Path) -> Thumbnails`
  (paths only, no I/O); `ensure_thumbnails` reuses it. Lets the scanner know paths without generating.

## 3. Producer–consumer thread pool (std threads + channel)

- **Decision**: A bounded pool of `std::thread` workers fed by a `std::sync::mpsc` channel. A producer
  enumerates photos needing thumbnails and sends jobs; each worker calls `photo::ensure_thumbnails`
  (which reuses valid existing thumbnails) and, on success, emits a `thumbnail-ready` event. Worker
  count is bounded by `std::thread::available_parallelism()` (e.g. cores − 1, min 1).
- **Rationale**: Literal "producer–consumer over a thread pool" (clarification), no new dependency
  (Principle X), explicit per-item progress + cancellation.
- **Alternatives considered**: `rayon` — rejected (new dependency; parallel-iterator model fits
  batch map/reduce, not cancellable producer–consumer with per-item events). `tokio::spawn_blocking`
  — rejected as the pool (not a bounded producer–consumer; harder backpressure/cancellation), though
  commands stay async and hand off to the pool.

## 4. Visible-first ordering

- **Decision**: The producer enqueues the open (visible) folder level's photos first, then walks
  deeper subfolders breadth-first. Workers therefore complete visible previews before deep ones.
- **Rationale**: Clarification (visible-first); better perceived speed on large trees.

## 5. Cancellation on folder switch

- **Decision**: Each generation run carries an `Arc<AtomicBool>` cancel flag held in the managed
  `FolderState`. Opening a new folder sets the previous run's flag; the producer stops enqueuing and
  workers stop pulling new jobs (an in-flight thumbnail finishes — cheap). 
- **Rationale**: FR-012 / SC-005; avoids wasted work and stale events after switching.

## 6. Progress events via Tauri Emitter

- **Decision**: Emit `thumbnail-ready` with a `{ path }` payload (the source photo path) as each
  thumbnail completes, using `app.emit(...)` — the same mechanism the watcher uses for
  `folder-changed`. The frontend maps `path` to its tree node.
- **Rationale**: Event-driven, no polling (clarification); reuses an established pattern.

## 7. Frontend progressive rendering

- **Decision**: `filesPanelStore` gains a reactive `readyThumbs` set of photo paths. `FilesPanel`
  listens for `thumbnail-ready` and adds the path. `FileTree` (content mode) renders
  `convertFileSrc(node.thumb_low)` only once the path is in `readyThumbs`, showing the existing icon
  placeholder before — avoiding broken-image flashes for not-yet-generated files.
- **Rationale**: Clean event-driven update with runes; `node.thumb_low` already carries the path.

## 8. State management

- **Decision**: Replace `WatcherState` with a `FolderState` (managed `Mutex`) holding the current
  watcher and the active run's cancel flag, so opening a new folder swaps both atomically.
- **Rationale**: One place owns the folder's live resources; mirrors the existing managed-state pattern.

## 9. Reuse of feature 002

- **Decision**: `get_thumbnails` stays as the viewer fallback and on-demand path; `ensure_thumbnails`
  (with its atomic writes and validity check) is the per-photo generator the pool calls.
- **Rationale**: No duplication; the folder module orchestrates, the photo module executes (Principle V / single responsibility).
