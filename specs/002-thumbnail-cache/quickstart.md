# Quickstart & Validation: Photo Thumbnail Cache

Run guide proving the feature end-to-end. See [contracts/](./contracts/) and
[data-model.md](./data-model.md) for details; implementation lives in `tasks.md` afterwards.

## Prerequisites

- Rust toolchain (edition 2021) and the project's npm deps installed.
- Test assets under `src-tauri/test_images/` (existing JPEG/PNG/WebP).

## Backend tests

```bash
cd src-tauri
cargo test --test thumbnail_test
```

Expected (`tests/thumbnail_test.rs`):

1. **Geometry** — generating low/high from a wide and a tall test image yields longest side 360 / 1920
   respectively, aspect ratio preserved. (FR-005, SC-007)
2. **No upscale** — a source whose longest side is < the target produces a thumbnail at the source size. (FR-005)
3. **Output** — thumbnails are valid JPG files under `<dir>/_thumbnail/` named `<file>.low.jpg` / `<file>.high.jpg`. (FR-003, FR-004, FR-012)
4. **Reuse** — a second `ensure_thumbnails` call regenerates nothing (file mtimes unchanged). (FR-002, SC-004)
5. **Regenerate invalid** — replacing a thumbnail with a 0-byte file causes regeneration on next call. (FR-011)
6. **Formats** — JPEG, PNG, and WebP sources all produce both thumbnails. (FR-013, SC-008)
7. **Size budget (sanity)** — low < 50 KB and high < 500 KB for a typical test photo. (SC-003)

## File-tree exclusion

- After a folder scan, confirm a `_thumbnail` subfolder is absent from the returned `FileNode` tree. (FR-008, SC-006)

## Frontend check

```bash
npx svelte-check --tsconfig ./tsconfig.json
```

## App smoke test

```bash
npm run tauri dev
```

1. Open a folder, switch the files panel to content view → each photo shows a small preview served from
   `_thumbnail` (not the full original); a `_thumbnail` folder appears on disk but not in the tree.
2. Click a photo → the viewer shows a loading indicator, then the 1920px preview. (FR-014)
3. Reopen the same photo → preview appears immediately, no new files written. (SC-001, SC-004)
4. Open a photo whose longest side is < 1920 → its high thumbnail is not upscaled. (FR-005)
