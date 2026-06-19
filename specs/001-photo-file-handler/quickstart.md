# Quickstart & Validation: Unified Photo File Handler

Run guide proving the feature end-to-end. See [contracts/](./contracts/) and
[data-model.md](./data-model.md) for details; implementation lives in `tasks.md` afterwards.

## Prerequisites

- Rust toolchain (edition 2021) and Node.js with the project's npm deps installed.
- Test assets under `src-tauri/test_images/` (e.g. `test_img_exif.jpg`); add small PNG and WebP
  fixtures for cross-format checks.

## Backend tests

```bash
cd src-tauri
cargo test                      # all integration tests
cargo test --test metadata_test # the photo module round-trip suite
```

Expected (extends `tests/metadata_test.rs`):

1. **Merge read** — a JPEG with values in different blocks returns the union; on a conflicting field the
   EXIF value wins (EXIF > IPTC > XMP). (SC-001, FR-003)
2. **Write-all-blocks round-trip** — set the four fields, save, reopen: values present in EXIF, IPTC,
   and XMP (format-permitting) and read back identically. (SC-002, FR-005)
3. **Empty-field removal** — clear a field, save, reopen: field is absent from every block. (SC-008, FR-015)
4. **Pixels preserved** — image pixel bytes are identical before/after save. (SC-003, FR-006)
5. **Unrelated tags preserved** — a pre-existing unmanaged tag survives a save. (FR-007)
6. **No-metadata file** — opening a stripped file yields empty fields without error. (SC-006, FR-010)
7. **PNG / WebP** — read/write round-trip on each; IPTC skipped where unsupported, XMP/EXIF still written. (SC-007, FR-011)
8. **decode_image** — returns a non-empty RGBA buffer; no file change, no metadata parse. (FR-009)

## Streaming / memory check

- Confirm `read_metadata` uses `little_exif::Metadata::new_from_path` (not `new_from_vec` / `fs::read`).
- Manual: read metadata of a large (≥100 MB) photo and observe peak memory comparable to a small file. (SC-004)

## Frontend check (only if any `.svelte` / `.ts` changed)

```bash
npx svelte-check --tsconfig ./tsconfig.json
```

## App smoke test

```bash
npm run tauri dev
```

1. Open a folder, select a JPEG → metadata panel shows merged fields.
2. Edit title/description/keywords/category, save → file updates losslessly; rename works if the
   filename stem changed.
3. Reopen the file (or inspect with ExifTool) → fields present in EXIF, IPTC, and XMP; cleared fields gone.
