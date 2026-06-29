# Quickstart / Validation: SQLite Intermediate Metadata Store

End-to-end checks that prove the feature. Contracts: [contracts/ipc-commands.md](./contracts/ipc-commands.md);
data model: [data-model.md](./data-model.md).

## Prerequisites

- Build the app: `npm install` then `npm run tauri dev` (the `bundled` SQLite compiles via the
  platform C toolchain — MSVC on Windows, gcc on Linux, clang on macOS).
- The store lives at `<app-data-dir>/metadata.db` (e.g. on Windows
  `%APPDATA%/loc.am.attributor/metadata.db`). Inspect it with any SQLite tool.
- Checks: backend `cd src-tauri && cargo test`; frontend `npx svelte-check --tsconfig ./tsconfig.json`.

## Scenario A — Attribution persists without rewriting the file (US1, SC-002/003)

1. Open a photo; note its file's bytes/mtime.
2. Run Ollama attribution.
3. **Expect**: fields fill; status shows **"in app"**; the photo file is byte-identical (SC-003);
   `metadata.db` has a row for the path with `synced=0`.
4. Close and reopen the app; reopen the photo.
5. **Expect**: the attributed metadata is shown from the store (SC-002), status still "in app",
   no file re-parse.

## Scenario B — Edits autosave to the store (US1, Q1)

1. Open a photo, type into title/keywords.
2. **Expect**: after the debounce, the store row updates (`synced=0`), status "in app"; the file
   is untouched.

## Scenario C — Fast reopen of an unchanged photo (SC-001)

1. Open a photo already in the store whose file is unchanged.
2. **Expect**: metadata appears in < 200 ms; resolution is `resolved`/store; the file's embedded
   metadata is not parsed (only the fingerprint is computed).

## Scenario D — Save commits to file and syncs the store (US2, SC-005)

1. With a photo in the "in app" state, click **Save**.
2. **Expect**: the file now contains the metadata; the store row becomes `synced=1` with a
   refreshed fingerprint; status changes to **"open"**.
3. Rename the file via the filename field + Save.
4. **Expect**: the store row key moves to the new path; one row, `synced=1`.

## Scenario E — Cancel reverts to the file (US2, SC-006)

1. With a photo in the "in app" state, click **Cancel** (between Ollama and Save).
2. **Expect**: fields and the store row return to the file's current metadata; status "open".
3. **Expect**: when the record is already `synced` and the form is clean, Cancel is disabled (FR-019).

## Scenario F — External change → conflict prompt (US3, SC-004)

1. Put a photo in the store (`synced=1`). Modify the same file with an external tool (content
   differs).
2. Reopen it in the app.
3. **Expect**: `open_metadata` returns `conflict`; a dialog asks **store vs file**.
   - Choose **file** → file metadata loads, store overwritten, `synced=1`.
   - Choose **store** → store metadata kept, fingerprint refreshed, `synced=1`.
4. Touch a file (change mtime only, content identical) and reopen.
5. **Expect**: still treated as a mismatch (all three must match) → conflict branch, per clarify Q2.

## Scenario G — Store-newer wins silently (US3 AC-2)

1. Photo with app-only edits (`synced=0`); also change the file externally.
2. Reopen.
3. **Expect**: no prompt — the store version loads (app data treated as newer), per the read-flow.

## Scenario H — Graceful degradation (FR-021)

1. Make the store unavailable (e.g. lock/remove `metadata.db` for the test build).
2. Open, edit, and save a photo.
3. **Expect**: the app still reads/writes file metadata directly; a store warning is logged; no
   crash.

## Batch checks

- Select multiple photos, run attribution → each row `synced=0`, files untouched; batch Save →
  each file written, rows `synced=1`.
- Force conflicts on several selected photos → a single **apply-to-all** choice resolves them
  (FR-020), no per-file prompt.
