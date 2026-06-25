# Contract: IPC Commands & Generation

Parameterized generation. All commands return `Result<T, String>`, never panic, camelCase payloads (§IX).

## Folder open / scan (changed signature)

The frontend derives `GenConfig` from settings and passes it; the backend scans the full tree (for
display) but generates only per the config, and only when something is eager.

```rust
#[serde(rename_all = "camelCase")]
struct GenConfig { low: bool, high: bool, recursive: bool }

#[tauri::command]
async fn open_folder(app, state, gen: GenConfig) -> Result<Option<FileNode>, String>;
#[tauri::command]
async fn open_folder_path(app, state, path: String, gen: GenConfig) -> Result<FileNode, String>;
#[tauri::command]
async fn scan_folder(app, state, path: String, gen: GenConfig) -> Result<FileNode, String>;
```

- Frontend derivation: `low = !lazy && smallThumbnails`, `high = !lazy && photoCaching`, `recursive = !currentFolderOnly`.
- Backend: `PhotoFolder::open/rescan` scan the tree, then start the pipeline **only if** `gen.low || gen.high`.
- `pipeline::start(app, root, cancel, low, high, recursive)` — `collect_jobs` walks only the top level when `!recursive`; each worker calls `ensure(path, low, high)`.

## On-demand generation (new / generalizes `get_thumbnails`)

```rust
#[tauri::command]
async fn cache_thumbnail(path: String, low: bool, high: bool) -> Result<Thumbnails, String>;
// spawn_blocking(|| photo::thumbnail::ensure(Path, low, high))
```

- Viewer open (when `cache.photo` on): `invoke('cache_thumbnail', {path, low: false, high: true})` → show `high`. Scope-free (FR-017).
- Lazy list show (when `cache.lazy && cache.smallThumbnails`): `invoke('cache_thumbnail', {path, low: true, high: false})` → add `path` to `readyThumbs`.

## Backend generation primitive

```rust
// photo/thumbnail.rs
pub fn ensure(source: &Path, low: bool, high: bool) -> Result<Thumbnails, String>;
pub fn ensure_thumbnails(source: &Path) -> Result<Thumbnails, String> { ensure(source, true, true) }
```

- Reuse valid cached sizes (`is_valid`); decode the source once iff a requested size is missing; generate only requested missing sizes.

## Events (unchanged)

`thumbnail-ready` still fires from the eager pipeline per completed photo. On-demand generation feeds
`readyThumbs` via the command return instead of an event.

## Frontend display gating

| Surface | Setting | Source |
|---------|---------|--------|
| Viewer (`+page.svelte` `showInViewer`) | `cache.photo` on | `cache_thumbnail` high → `convertFileSrc(high)`; fallback original until ready |
| Viewer | `cache.photo` off | `convertFileSrc(path)` directly, no generation |
| List (`FileTree.svelte`, `FilesPanel` icons) | `cache.smallThumbnails` on | cached low when in `readyThumbs`; lazy triggers `cache_thumbnail` low on show |
| List | `cache.smallThumbnails` off | `convertFileSrc(path)` directly |
