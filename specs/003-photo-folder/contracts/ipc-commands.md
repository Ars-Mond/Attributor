# Contract: Tauri IPC Commands

Signatures stay compatible with today's frontend; implementations move into the `folder` module and
`open_folder*` additionally start the thumbnail pipeline + watcher. Typed, `Result<T, String>`,
`#[serde(rename_all = "camelCase")]` (Principle IX).

## `open_folder`

```rust
#[tauri::command]
async fn open_folder(app: tauri::AppHandle) -> Result<Option<FileNode>, String>
```

- Picks a folder (dialog), scans it (fast — deterministic thumb paths, no generation), starts the
  watcher and the producer–consumer thumbnail pipeline (cancelling any previous run), returns the tree.
- `None` if the dialog is cancelled.

## `open_folder_path`

```rust
#[tauri::command]
async fn open_folder_path(app: tauri::AppHandle, path: String) -> Result<FileNode, String>
```

- Same as `open_folder` for a known path (session restore).

## `scan_folder`

```rust
#[tauri::command]
async fn scan_folder(app: tauri::AppHandle, path: String) -> Result<FileNode, String>
```

- Re-scans after a `folder-changed` event; returns the refreshed tree and schedules generation for
  newly added / still-missing thumbnails (reusing existing). (Now takes `app` to (re)drive the pipeline.)

## `get_thumbnails` (unchanged, feature 002)

```rust
#[tauri::command]
async fn get_thumbnails(path: String) -> Result<photo::Thumbnails, String>
```

- Viewer fallback / on-demand single-photo thumbnails. Retained.

## Registration

`tauri::generate_handler![ read_metadata, save_metadata, get_thumbnails, open_folder, open_folder_path, scan_folder, search_keywords ]`
— same set; `open_folder` / `open_folder_path` / `scan_folder` now receive `AppHandle` and route through `folder`.

## Notes

- `FileNode` is unchanged on the wire (`thumb_low` / `thumb_high` are the deterministic paths).
- Thumbnail completion is delivered out-of-band via the `thumbnail-ready` event (see events.md), not in the command result.
