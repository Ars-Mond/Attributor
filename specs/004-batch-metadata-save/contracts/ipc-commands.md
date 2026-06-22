# Contract: IPC Commands

New/changed Tauri commands. All return `Result<T, String>`, never panic (§IX). Registered in
`tauri::generate_handler![…]` in `lib.rs`.

## `save_metadata_batch` (new, async)

Writes a whole selection's metadata concurrently and streams progress.

```rust
#[tauri::command]
async fn save_metadata_batch(
    items: Vec<SaveRequest>,                       // resolved per-file metadata (frontend-merged)
    on_progress: tauri::ipc::Channel<BatchProgress>, // JS arg key: `onProgress`
    state: tauri::State<'_, BatchState>,
) -> Result<Vec<ItemStatus>, String>;
```

- **Behavior**: installs a fresh cancel flag in `state` (swap-out old), then
  `spawn_blocking(|| items.into_par_iter().enumerate().map(...).collect())`. Each item: check
  cancel → `Cancelled`, else `save_one(item)` → `Ok{path}` / `Failed{error}`; `on_progress.send(BatchProgress{index,status})`.
- **Returns**: `Vec<ItemStatus>` in input order — the authoritative final result (the channel is
  for incremental UI). `Err(String)` only for whole-operation failures (e.g. `spawn_blocking` join error).
- **Best-effort** (FR-003): a per-file failure never aborts the batch.
- **Frontend call**:
  ```ts
  import { Channel, invoke } from "@tauri-apps/api/core";
  const ch = new Channel<BatchProgress>();
  ch.onmessage = (m) => {/* update $state */};
  const results = await invoke<ItemStatus[]>("save_metadata_batch", {items, onProgress: ch});
  ```

## `cancel_batch` (new, sync)

Requests cancellation of the in-flight batch.

```rust
#[tauri::command]
fn cancel_batch(state: tauri::State<'_, BatchState>);
```

- Sets the current cancel flag to `true` (if any). Not-yet-started items resolve to `Cancelled`;
  an in-flight write completes (FR-017/FR-018). Idempotent; safe to call with no batch running.
- **Frontend call**: `await invoke("cancel_batch")` (wired to a Cancel button).

## `save_metadata` (existing, unchanged signature)

Single-file save. Internally refactored to delegate to the shared `batch::save_one`, so its
observable behavior is identical (FR-010). No contract change.

## Registration & managed state (lib.rs)

```rust
.manage(BatchState::default())
.invoke_handler(tauri::generate_handler![
    read_metadata, save_metadata, save_metadata_batch, cancel_batch,
    get_thumbnails, open_folder, open_folder_path, scan_folder, search_keywords,
])
```
