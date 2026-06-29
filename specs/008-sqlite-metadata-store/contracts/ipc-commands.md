# Phase 1 Contract: Tauri IPC Commands

All commands return `Result<T, String>` and use `#[serde(rename_all = "camelCase")]` on payload
types (Constitution IX). New/changed commands are registered in `src-tauri/src/lib.rs`.

## Shared payload types

```ts
// src/lib/types.ts (frontend mirror of the Rust types)

type SyncState = 'synced' | 'appOnly';

interface StoredMetadata {            // the editable fields held by the store
    title: string;
    description: string;
    keywords: string[];
    categories: string;
    releaseFilename: string;
}

type MetadataResolution =
    | { kind: 'resolved'; metadata: StoredMetadata; syncState: SyncState }
    | { kind: 'conflict'; store: StoredMetadata; file: StoredMetadata };
```

Rust mirror: `enum SyncState { Synced, AppOnly }` (camelCase), `struct StoredMetadata { … }`,
`enum MetadataResolution { Resolved { metadata, sync_state }, Conflict { store, file } }`
(`#[serde(tag = "kind", rename_all = "camelCase")]`).

## NEW commands

### `open_metadata(path: string) -> MetadataResolution`

Store-first resolution of one photo's metadata (read-flow, FR-006…FR-011). Computes the
fingerprint (hash authoritative) and returns either `resolved` (load proceeds) or `conflict`
(frontend must prompt). On a hash match with a differing mtime, it silently refreshes the stored
mtime and returns `resolved` — no prompt. Falls back to a plain file read returned as
`resolved`/`synced` on any store error (FR-021). Replaces the panel's direct `read_metadata` call
for single open.

### `apply_metadata_source(path: string, source: 'store' | 'file') -> MetadataResolution`

Finalizes a `conflict` from `open_metadata` (FR-012). `store` → keep store metadata, refresh
fingerprint; `file` → read file metadata, overwrite store **but retain the store's
`releaseFilename`** (no file equivalent). Always returns `resolved`.

### `store_metadata(path: string, fields: StoredMetadata) -> SyncState`

Upserts the working fields as **app-only** (`synced=0`) without touching the file (FR-013).
Called debounced by the editor on every field change and after single attribution. Returns the
new sync state (`appOnly`) so the UI can update the status. No-op-with-warning on store error.

### `revert_to_file(path: string) -> StoredMetadata`

Cancel / "revert to file" (FR-018): read the file's metadata, overwrite the store record to
mirror it (but **retain the store's `releaseFilename`** — no file equivalent), set `synced=1`, and
return the metadata for the frontend to reload into the form.

## CHANGED commands

### `read_metadata(path) -> ReadResult` *(unchanged signature; still used)*

Remains the raw file read. Now also used internally by the store (file side) and as the
fallback path. Batch file reads may keep using it where store resolution is not required.

### `save_metadata(metadata: SaveRequest) -> string` *(behavior added)*

After the existing file write via `batch::save_one`, **update the store**: refresh the
fingerprint from the written file and set `synced=1`; if the file was renamed, move the store
row to the returned final path. Return value (final path) unchanged.

### `save_metadata_batch(items, on_progress, state) -> ItemStatus[]` *(behavior added)*

Same store update as `save_metadata`, applied per successfully-written item.

### `attribute_batch(paths, config, on_progress, state)` *(behavior changed)*

`ollama/attribute.rs::attribute_and_save` no longer writes the file; instead it **upserts each
result into the store as app-only** (`synced=0`). `attribute_photo` (single) is unchanged at the
IPC level — the frontend persists the returned result via the debounced `store_metadata` path.

## Backend wiring

- `src-tauri/src/lib.rs`: `.manage(store::DbState::new(...))` built in `setup` from
  `app.path().app_data_dir()`; register `open_metadata`, `apply_metadata_source`,
  `store_metadata`, `revert_to_file` in `generate_handler!`.
- All store commands run their SQLite work inside `tokio::task::spawn_blocking`.

## Frontend wrappers (`src/lib/store/metadata.ts`)

Typed `invoke` wrappers: `openMetadata(path)`, `applyMetadataSource(path, source)`,
`storeMetadata(path, fields)`, `revertToFile(path)` — mirroring the commands above.
