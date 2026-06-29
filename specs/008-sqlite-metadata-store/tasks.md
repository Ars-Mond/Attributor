# Tasks: SQLite Intermediate Metadata Store

**Input**: Design documents from `/specs/008-sqlite-metadata-store/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/ipc-commands.md, quickstart.md

**Tests**: Lean Rust unit tests are included for the pure store logic (fingerprint, read-flow
resolution, conflict resolution, fallback) because plan.md's Testing section calls for them and
the repo already has a Rust test harness. Frontend changes are validated with `npx svelte-check`.

**Organization**: Tasks are grouped by user story. US1 (single-photo store-first persistence) is
the MVP; US2 adds Save/Cancel; US3 adds conflict resolution and all batch handling.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: US1 / US2 / US3 for user-story phases
- Exact file paths are given in each task

## Path Conventions

Tauri desktop project: Rust backend in `src-tauri/src/`, Svelte 5 frontend in `src/`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Add dependencies and create the empty module skeleton.

- [x] T001 Add the new crates to `src-tauri/Cargo.toml`: `rusqlite = { version = "0.32", features = ["bundled"] }` and `xxhash-rust = { version = "0.8", features = ["xxh3"] }` (keep the rest unchanged); run `cargo check` to confirm the `bundled` SQLite compiles.
- [x] T002 Create the store module skeleton with empty/stub items and wire it in: `src-tauri/src/store/mod.rs`, `src-tauri/src/store/schema.rs`, `src-tauri/src/store/record.rs`, `src-tauri/src/store/fingerprint.rs`, and add `mod store;` to `src-tauri/src/lib.rs`.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The store engine, types, fingerprint, schema, connection, CRUD, and app wiring that
every user story depends on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [x] T003 [P] Implement `fingerprint::compute(path) -> Result<Fingerprint, String>` in `src-tauri/src/store/fingerprint.rs`: read `std::fs::metadata` for size + mtime (Unix nanos as `i64`) and stream the whole file in 64 KiB chunks through xxh3-64; define `struct Fingerprint { size: u64, mtime: i64, hash: u64 }` with a **hash-match** helper (`hash` is authoritative for content identity; a differing mtime is not a change). Log on read error.
- [x] T004 [P] Define the record/IPC types in `src-tauri/src/store/record.rs`: `StoredMetadata { title, description, keywords: Vec<String>, categories, release_filename }`, `enum SyncState { Synced, AppOnly }`, `StoreRecord` (metadata + fingerprint + sync state + timestamps), and `enum MetadataResolution { Resolved { metadata, sync_state }, Conflict { store, file } }` — all `#[serde(rename_all = "camelCase")]`, with `MetadataResolution` tagged `#[serde(tag = "kind", rename_all = "camelCase")]` (per contracts/ipc-commands.md).
- [x] T005 Implement the schema in `src-tauri/src/store/schema.rs`: `init(conn)` runs `CREATE TABLE IF NOT EXISTS photo_metadata (...)` exactly per data-model.md (columns, defaults, PK on `path`) and sets `PRAGMA journal_mode=WAL; synchronous=NORMAL`.
- [x] T006 Implement `DbState` and CRUD in `src-tauri/src/store/mod.rs` (depends on T004, T005): `DbState { conn: Mutex<rusqlite::Connection> }`; an `open(db_path)` that opens the connection and calls `schema::init`; helpers `get_by_path`, `upsert_app_only`, `mark_synced_with_fingerprint`, `move_path`, `read_record`; keywords (de)serialized as JSON via `serde_json`; a `with_db<T>(state, f)` wrapper that logs and signals fallback on any error (FR-021/FR-022). All callers will run these inside `spawn_blocking`.
- [x] T007 Wire the store into the app in `src-tauri/src/lib.rs` (depends on T006): in `tauri::Builder::setup`, resolve `app.path().app_data_dir()`, create the dir if needed, build `DbState::open(dir.join("metadata.db"))`, log success/failure, and `.manage(db_state)`. Leave a placeholder in `generate_handler!` for the store commands added per story.
- [x] T008 [P] Create the frontend types + wrapper skeleton: add `SyncState`, `StoredMetadata`, `MetadataResolution` to `src/lib/types.ts`, and create `src/lib/store/metadata.ts` with empty typed `invoke` wrappers (filled per story). Run `npx svelte-check`.

**Checkpoint**: Store opens on launch, schema exists, types compile.

---

## Phase 3: User Story 1 - Edits & attribution kept by the app (Priority: P1) 🎯 MVP

**Goal**: Opening a single photo resolves metadata store-first; manual edits and single-photo
Ollama attribution persist to the store immediately (app-only) without touching the file; a new
"saved in app" status shows; data survives a restart.

**Independent Test**: Open a photo, run attribution or edit fields → file byte-unchanged, status
"in app", row in `metadata.db` with `synced=0`; restart the app and reopen → metadata loads from
the store (quickstart Scenarios A, B, C).

- [x] T009 [US1] Implement the `open_metadata(path) -> MetadataResolution` command in `src-tauri/src/store/mod.rs` and register it in `src-tauri/src/lib.rs` (depends on T006, T007): apply the read-flow from data-model.md — no record → read file (via `crate::photo::read_metadata`) + insert (`synced=1`) → `Resolved/synced`; **hash match** → silently refresh the stored mtime if it differs, then `Resolved` with the stored sync state; **hash differs** with `synced=0` → `Resolved/appOnly` (store wins); **hash differs** with `synced=1` → `Conflict { store, file }`. Fall back to a plain file read returned as `Resolved/synced` on store error.
- [x] T010 [US1] Implement the `store_metadata(path, fields) -> SyncState` command in `src-tauri/src/store/mod.rs` and register it in `src-tauri/src/lib.rs` (depends on T006, T007): upsert the fields as `synced=0` (app-only), do not touch the file, bump `updated_at`, return `AppOnly`; no-op-with-warning on store error.
- [x] T011 [P] [US1] Fill the `openMetadata(path)` and `storeMetadata(path, fields)` typed wrappers in `src/lib/store/metadata.ts` (depends on T008).
- [x] T012 [US1] Switch single-file loading to the store in `src/lib/panel/MetadataPanel.svelte` `loadFile()`: call `openMetadata` instead of `read_metadata`; on `resolved` populate fields + track `syncState`; on `conflict` default to the file version and log a warning (the prompt arrives in US3) (depends on T011).
- [x] T013 [US1] Add the debounced app-only persistence in `src/lib/panel/MetadataPanel.svelte`: an `$effect` that, when a single file is open and the working fields change (including right after single attribution fills them), calls `storeMetadata` after the existing autosave-style debounce and updates the tracked `syncState` to `appOnly` (depends on T011, T012).
- [x] T014 [P] [US1] Add the new file status in `src/lib/panel/MetadataPanel.svelte`: extend the `fileStatus` `$derived` to yield `app` when the open record's `syncState === 'appOnly'` (precedence per data-model.md: none → edit → app → open), and add a `status-dot--app` / `status-label--app` color from theme tokens (in the component styles).
- [x] T015 [P] [US1] Add the status i18n keys `metadata.fileStatus.app` (EN "in app", RU "в приложении") in `src/lib/i18n/en.ts`, `src/lib/i18n/ru.ts`, and `src/lib/i18n/types.ts`.
- [x] T016 [P] [US1] Add Rust unit tests (in `src-tauri/src/store/fingerprint.rs` and `src-tauri/src/store/mod.rs` `#[cfg(test)]`, or `src-tauri/tests/store_test.rs`): hash match treated as unchanged including the mtime-only case (silent mtime refresh, no conflict) vs a hash difference; resolution decisions for no-record, hash-match, and store-newer (`synced=0`) cases using a temp `:memory:`/tempfile DB.

**Checkpoint**: Single-photo store-first persistence works end to end and survives restart.

---

## Phase 4: User Story 2 - Commit to file, or revert to file (Priority: P2)

**Goal**: Save writes the store's metadata into the photo file and marks the store synced; a new
Cancel button (between Ollama and Save) reverts the working fields and the store record to the file.

**Independent Test**: With a photo in the "in app" state, Save → file contains the metadata, status
becomes "open", row `synced=1`; Cancel on another → fields + row return to the file, status "open";
Cancel disabled when already synced and clean (quickstart Scenarios D, E).

- [x] T017 [US2] Update the store after a single file write in `src-tauri/src/lib.rs` `save_metadata` (using a helper in `src-tauri/src/store/mod.rs`, depends on T006): after `batch::save_one` returns the final path, recompute the fingerprint from the written file and `mark_synced_with_fingerprint`; if the file was renamed, `move_path(old, new)` first. Log; tolerate store errors (file write already succeeded).
- [x] T018 [US2] Implement `revert_to_file(path) -> StoredMetadata` in `src-tauri/src/store/mod.rs` and register it in `src-tauri/src/lib.rs` (depends on T006, T007): read file metadata, overwrite the store record to mirror it (Reset **clears** the store-only fields — release_filename + flags), set `synced=1` with the current fingerprint, and return the metadata.
- [x] T019 [P] [US2] Fill the `revertToFile(path)` typed wrapper in `src/lib/store/metadata.ts` (depends on T008).
- [x] T020 [US2] Add the Cancel control between the Ollama and Save buttons in `src/lib/panel/MetadataPanel.svelte` (single mode footer): on click call `revertToFile`, reload the returned fields, set status to synced; disable it when the record is `synced` and the form is not dirty (FR-019) (depends on T019).
- [x] T021 [US2] After a successful Save in `src/lib/panel/MetadataPanel.svelte` (`doSave`), set the tracked `syncState` to `synced` so the status flips to "open" (depends on T012).
- [x] T022 [P] [US2] Add the Cancel button i18n keys (e.g. `metadata.button.revert` + title) in `src/lib/i18n/en.ts`, `src/lib/i18n/ru.ts`, `src/lib/i18n/types.ts`.
- [x] T023 [P] [US2] Add Rust tests in `src-tauri/tests/store_test.rs` (or `#[cfg(test)]`): save → store `synced=1` with refreshed fingerprint; rename moves the row; `revert_to_file` overwrites the record from the file.

**Checkpoint**: Single-photo commit-to-file and revert-to-file round trip works.

---

## Phase 5: User Story 3 - Detect & resolve external changes + batch (Priority: P3)

**Goal**: When `open_metadata` returns a conflict, prompt the user (store vs file) and finalize the
choice; bring batch operations into the store model (store-first batch load, batch attribution to
the store, batch save sync) with a single apply-to-all conflict resolution.

**Independent Test**: Modify a stored photo's file externally and reopen → store-vs-file prompt;
choose file/store and verify the right source loads and `synced=1`; an mtime-only touch does NOT prompt (silent mtime refresh);
batch conflicts resolve with one apply-to-all choice (quickstart Scenarios F, G + batch checks).

- [x] T024 [US3] Implement `apply_metadata_source(path, source) -> MetadataResolution` in `src-tauri/src/store/mod.rs` and register it in `src-tauri/src/lib.rs` (depends on T009): `store` → refresh fingerprint only, return `Resolved/synced`; `file` → read file, overwrite record **but retain the stored `release_filename`**, `synced=1`, return `Resolved/synced`.
- [x] T025 [P] [US3] Fill the `applyMetadataSource(path, source)` typed wrapper in `src/lib/store/metadata.ts` (depends on T008).
- [x] T026 [US3] Single-photo conflict prompt in `src/lib/panel/MetadataPanel.svelte`: when `openMetadata` returns `conflict`, show a `ConfirmDialog` (reused primitive) offering "keep store" vs "keep file", then call `applyMetadataSource` and load the result (replaces the US1 default-to-file behavior) (depends on T012, T025).
- [x] T027 [US3] Make batch loading store-first in `src/lib/panel/MetadataPanel.svelte` `loadBatchData()`: resolve each path via `openMetadata` (instead of `read_metadata`), using stored metadata for the batch union (depends on T011).
- [x] T028 [US3] Change batch attribution to persist to the store in `src-tauri/src/ollama/attribute.rs` `attribute_and_save`: instead of writing the file, upsert each result into the store as app-only (`synced=0`) via `DbState`; thread `DbState` through `attribute_batch` in `src-tauri/src/ollama/mod.rs` (depends on T006).
- [x] T029 [US3] Sync the store after batch file saves in `src-tauri/src/batch/mod.rs` (`save_metadata_batch`): for each `Ok` item, `mark_synced_with_fingerprint` (and `move_path` on rename) via the T017 helper (depends on T017).
- [x] T030 [US3] Batch apply-to-all conflict resolution in `src/lib/panel/MetadataPanel.svelte`: collect photos that returned `conflict` during batch load, present one apply-to-all choice (store vs file), then call `applyMetadataSource` per file (FR-020) (depends on T026, T027).
- [x] T031 [P] [US3] Add the conflict-dialog i18n keys (title, body, "keep store", "keep file", batch apply-to-all) in `src/lib/i18n/en.ts`, `src/lib/i18n/ru.ts`, `src/lib/i18n/types.ts`.
- [x] T032 [P] [US3] Add Rust tests in `src-tauri/tests/store_test.rs`: `apply_metadata_source` for both `store` and `file` (the `file` branch retains `release_filename`); a conflict is produced for a synced record whose file content changed (hash differs), while an mtime-only touch does NOT produce a conflict.

**Checkpoint**: External-change conflicts and all batch flows work in the store model.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Robustness, validation, and final green checks.

- [x] T033 Audit graceful degradation across `src-tauri/src/store/mod.rs` and `src-tauri/src/lib.rs`: confirm every store command uses `with_db` so a store failure logs and falls back to direct file read/write (FR-021), and that every store op + error is logged (FR-022).
- [x] T034 [P] Add a Rust test in `src-tauri/tests/store_test.rs` for the store-unavailable fallback path (open/edit/save still work when the DB cannot be opened).
- [ ] T035 Execute the quickstart.md scenarios A–H and the batch checks manually; fix any gaps found.
- [x] T036 [P] Final green gate: `npx svelte-check --tsconfig ./tsconfig.json` (0 issues) and `cd src-tauri && cargo test` (all pass); confirm the Constitution Principle I exception note is present in plan.md Complexity Tracking.

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (Phase 1)**: no dependencies.
- **Foundational (Phase 2)**: depends on Setup; **blocks all user stories**.
- **US1 (Phase 3)**: depends on Foundational. MVP.
- **US2 (Phase 4)**: depends on Foundational; independent of US1 at the file level but shares the panel — sequence after US1 in a single-developer flow.
- **US3 (Phase 5)**: depends on Foundational; reuses `open_metadata` (T009) and the save-sync helper (T017).
- **Polish (Phase 6)**: depends on all desired stories.

### Key task dependencies

- T003, T004 → T005 → T006 → T007 (foundational chain); T008 parallel to all of these.
- US1: T009, T010 (after T006/T007) → T011 → T012 → T013; T014, T015, T016 parallel.
- US2: T017, T018 (after T006/T007) → T019 → T020; T021, T022, T023 parallel.
- US3: T024 (after T009) → T025 → T026; T027 after T011; T028 after T006; T029 after T017; T030 after T026/T027.

### Parallel opportunities

- Foundational: **T003** and **T004** together; **T008** alongside them.
- US1: **T014**, **T015**, **T016** together (different files) once T012/T013 land.
- US2: **T019**, **T022**, **T023** together.
- US3: **T025**, **T031**, **T032** together.
- Polish: **T034**, **T036** together.

---

## Parallel Example: User Story 1

```text
# After T012/T013, run these together (different files):
T014  New "app" status in MetadataPanel.svelte styles + derived
T015  Status i18n keys in en.ts / ru.ts / types.ts
T016  Rust unit tests for fingerprint + resolution
```

---

## Implementation Strategy

### MVP first (US1 only)

1. Phase 1 Setup → 2. Phase 2 Foundational → 3. Phase 3 US1 → **STOP & validate** (quickstart A/B/C):
single-photo store-first persistence + "in app" status, surviving restart.

### Incremental delivery

- US1 → MVP (store is the working layer for single photos).
- US2 → commit-to-file + revert (full single-photo round trip).
- US3 → external-change conflicts + all batch flows.
- Polish → fallback hardening + quickstart + green checks.

---

## Notes

- Run `npx svelte-check` after each frontend edit (constitution Development Workflow); run
  `cargo check`/`cargo test` after backend edits.
- Commit per Spec Kit phase (constitution Principle VII) — the implementation phase is one commit.
- **Behavior change carried by US3 (T028)**: batch Ollama attribution stops writing files and writes
  the store instead (FR-013) — call this out when implementing so 007's batch flow is updated knowingly.
- Store-only fields (`releaseFilename` + the 3 flags) live only in the store. Every DB update
  preserves them (conflict→"file" via T024, batch save/attribution) EXCEPT the Reset button (T018),
  which clears them. See data-model.md.
- [P] = different files, no incomplete dependencies. Avoid two [P] tasks touching
  `MetadataPanel.svelte` simultaneously — they share the file and must be sequenced.
