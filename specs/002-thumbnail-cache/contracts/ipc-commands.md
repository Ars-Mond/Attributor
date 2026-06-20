# Contract: Tauri IPC Command

Thumbnail paths are delivered **primarily via the folder scan**: `scan_dir` fills each image
`FileNode` with `thumb_low` / `thumb_high`. The command below is a **viewer fallback** for files
opened outside a scan. Existing commands are unchanged. Typed, `Result<T, String>`, never panics
across the boundary, `#[serde(rename_all = "camelCase")]` (Principle IX).

## `get_thumbnails`

```rust
#[tauri::command]
async fn get_thumbnails(path: String) -> Result<Thumbnails, String>
```

- **In**: absolute path to a source photo (JPEG/PNG/WebP).
- **Out**: `Thumbnails { low: String, high: String }` — absolute paths to the two JPG thumbnails.
- **Behavior**: Ensures both variants exist under `<dir>/_thumbnail/` (creating the folder and
  generating any missing/invalid thumbnail), then returns their paths. If both already exist and are
  valid, returns immediately without regenerating. CPU-bound work runs off the UI thread
  (`tokio::task::spawn_blocking`) so the UI never freezes (FR-009).
- **Errors**: unsupported/corrupt source, or a write failure (permissions, disk full) → `Err(String)`
  (logged). A failure for one photo does not affect others.
- **Idempotent**: repeated calls for the same photo return the same paths and regenerate nothing while
  the thumbnails remain valid.

## Registration

`tauri::generate_handler![ read_metadata, save_metadata, get_thumbnails, open_folder, open_folder_path, scan_folder, search_keywords ]`
— `get_thumbnails` added to the existing set.

## Frontend usage

- File hierarchy (content mode): render `convertFileSrc(node.thumb_low)` (from the scan) as the row
  thumbnail, replacing today's `convertFileSrc(node.path)` on the full original. No per-row IPC.
- Viewer: render `convertFileSrc(node.thumb_high)` for the active node. If `thumb_high` is absent
  (file opened outside a scan), call `get_thumbnails(activePath)` and show a loading indicator until
  it resolves. Nothing is written to disk beyond the thumbnail files themselves.
