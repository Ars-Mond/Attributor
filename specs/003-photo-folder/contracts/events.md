# Contract: Tauri Events

## `thumbnail-ready` (new)

Emitted by a pipeline worker each time a photo's thumbnails finish (or are confirmed present).

```rust
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ThumbnailReady { path: String }   // source photo path

app.emit("thumbnail-ready", ThumbnailReady { path })?;
```

- **Payload**: `{ path: string }` — the source photo path; the frontend maps it to its tree node.
- **Frontend**: `listen<{path: string}>("thumbnail-ready", e => readyThumbs.add(e.payload.path))`;
  `FileTree` then renders that node's low thumbnail (placeholder shown before).
- **Cadence**: one per photo whose thumbnails became ready; not emitted for a folder whose run was
  cancelled (folder switched away). Already-valid thumbnails may emit immediately (reuse path).

## `folder-changed` (existing, unchanged)

Emitted by the `notify` watcher when the open folder changes; the frontend debounces and calls
`scan_folder`. Behavior preserved by this feature.
