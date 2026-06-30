# Quickstart & Validation: Photo Folder Handler

Run guide proving the feature end-to-end. See [contracts/](./contracts/) and
[data-model.md](./data-model.md); implementation lives in `tasks.md` afterwards.

## Prerequisites

- Rust toolchain (edition 2021) and the project's npm deps installed.
- Test assets under `src-tauri/test_images/` plus a temp folder with several photos and subfolders.

## Backend tests

```bash
cd src-tauri
cargo test --test folder_test
```

Expected (`tests/folder_test.rs`):

1. **Scan tree** — scanning a folder with subfolders returns all folders/photos; `_thumbnail` folders are excluded. (FR-002, FR-010, SC-006)
2. **Deterministic paths, no generation** — after a scan, each photo node has `thumb_low`/`thumb_high` set, and no thumbnail files were created by the scan itself. (FR-002)
3. **Enumerate photos** — `photo_paths` returns 100% of supported photos across the tree, 0% `_thumbnail`. (FR-007, SC-006)
4. **Pipeline generates + reuses** — running the pipeline over a folder creates missing thumbnails (files appear in `_thumbnail`) and reuses existing ones (no rewrite of valid files). (FR-003, FR-005, SC-004)
5. **Concurrency** — generation uses multiple workers (bounded); for N photos it is meaningfully faster than sequential. (FR-004, SC-002)
6. **Cancellation** — flipping the cancel flag stops the producer/workers promptly; remaining photos are not generated. (FR-012, SC-005)
7. **Graceful failure** — a corrupt photo logs and is skipped; the run continues for the rest. (FR-011)

## Frontend check

```bash
npx svelte-check --tsconfig ./tsconfig.json
```

## App smoke test

```bash
npm run tauri dev
```

1. Open a folder with many photos / subfolders → the structure appears immediately and is navigable
   while previews fill in. (SC-001, SC-003)
2. Watch previews populate the **visible** level first, then deeper subfolders. (SC-008)
3. Open the same folder again → previews appear quickly from cache (no regeneration). (SC-004)
4. Switch to another folder mid-generation → the previous folder's generation stops (CPU drops). (SC-005)
5. Add a photo on disk → it appears (`folder-changed`) and receives a thumbnail. (FR-009)
