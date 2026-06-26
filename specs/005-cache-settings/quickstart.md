# Quickstart & Validation: Configurable Photo Caching

Build, then validate the four toggles end-to-end. Details in [contracts/](./contracts/) and
[data-model.md](./data-model.md).

## Prerequisites

- No new dependencies. Standard build:

```bash
cargo build --manifest-path src-tauri/Cargo.toml
cargo test  --manifest-path src-tauri/Cargo.toml
npx svelte-check --tsconfig ./tsconfig.json
```

## Settings under test

Settings → "Caching": Photo caching (default off), Cache small thumbnails (default off), Lazy caching
(default off), Current folder only (default on).

## Validation scenarios

### S1 — Defaults: nothing cached, originals shown (US1/US2, SC-001/002/003)

1. Fresh settings. Open a folder of photos and open one in the viewer.
2. Expect: no `_thumbnail` files created; the viewer shows the original; the list shows originals/icons.
3. Backend test: `pipeline` not started when `GenConfig{low:false, high:false}`.

### S2 — Photo caching on → viewer uses high (US1, SC-002 inverse)

1. Turn Photo caching on. Open a photo in the viewer.
2. Expect: the viewer shows the cached high thumbnail (generated if missing); with it off again, the
   viewer shows the original and no high is generated.

### S3 — Small thumbnails independent (US2, SC-003 inverse)

1. Turn Cache small thumbnails on (Photo caching off). Browse a folder.
2. Expect: the list shows cached small previews; the viewer still shows originals (independent).
3. Backend test: `ensure(path, low=true, high=false)` writes only the low file.

### S4 — Both on, lazy off → single decode (SC-004)

1. Photo caching on + Cache small thumbnails on + Lazy off. Open a folder.
2. Expect: each photo's low and high are produced from one decode of the source.
3. Backend test: `ensure(path, true, true)` decodes once and writes both; `thumbnail_test` asserts both
   sizes valid.

### S5 — Lazy on → nothing at folder open; on-display generation (US3, SC-005)

1. Lazy on (Photo caching + small on). Open a folder.
2. Expect: 0 thumbnails generated at open. Showing a list item in the hierarchy generates its small;
   opening a photo in the viewer generates its large.

### S6 — Current folder only (US4, SC-006, FR-017)

1. Current folder only on (default). Open a folder containing subfolders, with eager caching on.
2. Expect: only top-level photos are cached; subfolder photos are not auto-cached. **But** explicitly
   opening a subfolder photo in the viewer still generates its high thumbnail.
3. Turn it off → subfolder photos are auto-cached too.
4. Backend test: `pipeline` with `recursive=false` enqueues only top-level photos; `recursive=true`
   enqueues the subtree.

### S7 — Persistence & non-destructive toggles (SC-007/008)

1. Change settings, restart the app → settings retained, behavior matches without restart.
2. Toggle a setting off then on → existing valid cache files are not deleted (reused).
