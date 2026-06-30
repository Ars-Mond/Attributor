---
description: "Task list for Photo Folder Handler"
---

# Tasks: Photo Folder Handler

**Input**: Design documents from `specs/003-photo-folder/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: Included — the project has an integration-test suite and `quickstart.md` enumerates
folder validation scenarios. Test inputs are generated in-test via the `image` crate; existing
fixtures under `src-tauri/test_images/` are also used.

**Organization**: Grouped by user story — US1 (fast open & browse), US2 (concurrent thumbnail
creation), US3 (folder operations & queries). A `PhotoFolder` struct (the class, mirroring `Photo`)
consolidates `filetree.rs` into a `folder` module and delegates per-photo work to the `photo`
module (single responsibility). Concurrency uses `std::thread` + `std::sync::mpsc` — no new
dependency (constitution v1.1.0 §VIII permits an equivalent thread pool).

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different file, no dependency on an incomplete task)
- **[Story]**: US1 / US2 / US3 (user-story phases only)

## Path Conventions

Backend Rust under `src-tauri/src/`; integration tests under `src-tauri/tests/`; frontend under `src/lib/`.

---

## Phase 1: Setup (Shared Infrastructure)

- [x] T001 Create the `folder/` module: `src-tauri/src/folder/mod.rs` skeleton (`PhotoFolder` struct, `FileNode` moved here, `FolderState`, `mod scan; mod pipeline; mod watch;` declarations) and declare `mod folder;` in `src-tauri/src/lib.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared low-level pieces every story builds on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [x] T002 [P] Add `pub fn thumbnail_paths(source: &Path) -> Thumbnails` (deterministic paths, no file I/O) in `src-tauri/src/photo/thumbnail.rs`, refactor `ensure_thumbnails` to reuse it, and re-export from `src-tauri/src/photo/mod.rs`
- [x] T003 [P] Define the `PhotoFolder` struct (entry point for folder ops), `FileNode` (with `thumb_low`/`thumb_high`), and `FolderState` (`current`, `watcher`, `cancel: Option<Arc<AtomicBool>>`) in `src-tauri/src/folder/mod.rs`

**Checkpoint**: Module, `PhotoFolder`, and the paths-only helper exist.

---

## Phase 3: User Story 1 - Open a folder and browse immediately (Priority: P1) 🎯 MVP

**Goal**: Selecting a folder shows its structure (folders + photos) immediately, without generating thumbnails first.

**Independent Test**: Scan a folder with subfolders → the tree is returned quickly with `_thumbnail` excluded and deterministic `thumb_low/high` set, and the scan itself creates no thumbnail files.

- [x] T004 [US1] Implement `scan_dir` in `src-tauri/src/folder/scan.rs` — build the `FileNode` tree, exclude `_thumbnail` folders, fill `thumb_low/high` from `photo::thumbnail::thumbnail_paths` (no generation)
- [x] T005 [US1] Move the `notify` watcher into `src-tauri/src/folder/watch.rs` (emit `folder-changed`), storing the watcher in `FolderState`
- [x] T006 [US1] Implement `PhotoFolder::open` / `open_path` / `rescan` in `src-tauri/src/folder/mod.rs` (cancel previous run, scan, start watcher; pipeline hook wired in US2) and register `FolderState` as managed state
- [x] T007 [US1] Route `open_folder` / `open_folder_path` / `scan_folder` commands to `PhotoFolder` (pass `AppHandle`, use `FolderState`) and remove `filetree.rs` + `WatcherState` in `src-tauri/src/lib.rs`
- [x] T008 [P] [US1] Tests: scan returns the tree with `_thumbnail` excluded, deterministic `thumb_low/high` set, and zero thumbnail files created by the scan, in `src-tauri/tests/folder_test.rs`

**Checkpoint**: Opening a folder returns the structure fast; nothing blocks on thumbnails.

---

## Phase 4: User Story 2 - Concurrent thumbnail creation (Priority: P2)

**Goal**: A producer–consumer thread pool generates the folder's thumbnails in parallel, visible-level first, reusing existing ones, with the UI updating each preview as it becomes ready.

**Independent Test**: Open a folder of photos → thumbnails are produced by multiple workers, files appear in `_thumbnail`, existing ones are reused, and `thumbnail-ready` events drive progressive previews; switching folders stops the previous run.

- [x] T009 [US2] Implement `src-tauri/src/folder/pipeline.rs` — producer (visible level first, then deeper subfolders) + bounded `std::thread` pool over an `mpsc` channel + `Arc<AtomicBool>` cancellation; each worker calls `photo::ensure_thumbnails`
- [x] T010 [US2] Emit a `thumbnail-ready` event (`{ path }`, camelCase) per completed photo and start the pipeline from `PhotoFolder::open` (new run + cancel flag) in `src-tauri/src/folder/pipeline.rs` and `src-tauri/src/folder/mod.rs`
- [x] T011 [P] [US2] Frontend progressive previews: add a reactive `readyThumbs` set in `src/lib/panel/filesPanelStore.svelte.ts`, listen for `thumbnail-ready` in `src/lib/panel/FilesPanel.svelte`, and render `convertFileSrc(node.thumb_low)` only once ready (placeholder before) in `src/lib/reusable/FileTree.svelte`
- [x] T012 [P] [US2] Tests: pipeline generates missing + reuses valid thumbnails, runs multiple workers, and stops on cancellation, in `src-tauri/tests/folder_test.rs`
- [x] T013 [P] [US2] Test (visible-first, FR-016): the producer enqueues the visible folder level's photos before deeper subfolders' (assert deterministic enqueue order, not timing), in `src-tauri/tests/folder_test.rs`

**Checkpoint**: Folder thumbnails generate concurrently and appear progressively; US1 + US2 work together.

---

## Phase 5: User Story 3 - Folder operations and queries (Priority: P3)

**Goal**: `PhotoFolder` is the single place for enumerating/searching photos, locating thumbnails, and reflecting on-disk changes (new photos get thumbnails).

**Independent Test**: With a folder open, enumerate all photo paths (excluding `_thumbnail`); add a photo on disk → it appears and gets a thumbnail.

- [x] T014 [US3] Implement `PhotoFolder` queries — `photo_paths(&FileNode) -> Vec<String>`, locate/search photos and their thumbnails — in `src-tauri/src/folder/mod.rs`
- [x] T015 [US3] On `folder-changed` rescan, schedule generation for newly added / still-missing thumbnails (reuse existing) via the pipeline in `src-tauri/src/folder/mod.rs`
- [x] T016 [P] [US3] Tests: `photo_paths` returns all photos and 0 `_thumbnail` entries; a corrupt photo is skipped (logged) without aborting the run, in `src-tauri/tests/folder_test.rs`
- [x] T017 [P] [US3] Test (FR-009): rescanning after a photo is added on disk schedules a thumbnail for it (drive the rescan→scheduling path directly, no OS watcher), in `src-tauri/tests/folder_test.rs`

**Checkpoint**: All three user stories independently functional.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [x] T018 [P] Logging audit across `src-tauri/src/folder/` — error-site logging (scan, worker, watcher), concise English, no `println!`/`dbg!`
- [x] T019 [P] Run `npx svelte-check --tsconfig ./tsconfig.json` for the changed frontend files and resolve any issues
- [x] T020 Run `cargo test` (all green) and validate the `quickstart.md` scenarios

---

## Dependencies & Execution Order

- **Setup (T001)** → **Foundational (T002–T003)** → user stories.
- **US1 (T004–T008)** depends on Foundational; `scan_dir` (T004) uses `thumbnail_paths` (T002); `PhotoFolder::open` (T006) depends on T004/T005; command wiring (T007) depends on T006.
- **US2 (T009–T013)** depends on US1 (it starts the pipeline from `PhotoFolder::open`); T010 depends on T009; T011 (frontend) is independent of the backend files.
- **US3 (T014–T017)** depends on US1 (tree/commands) and reuses the US2 pipeline for T015.
- **Polish (T018–T020)** last; T020 after all implementation.
- Within `folder/mod.rs`: T003 → T006 → T010 → T014/T015 (same file, sequential).

## Parallel Opportunities

- Foundational: T002 (photo) ∥ T003 (folder) — different files.
- Each story's test task runs parallel to that story's implementation (different files): T008 ∥ T004–T007; T012/T013 ∥ T009–T010; T016/T017 ∥ T014–T015.
- US2 frontend (T011) runs parallel to US2 backend (T009/T010).
- Polish: T018 (Rust) ∥ T019 (frontend).
- Note: test tasks T008/T012/T013/T016/T017 share `folder_test.rs` → sequential among themselves (group within a phase, different phases anyway).

## Implementation Strategy

- **MVP** = Setup + Foundational + **User Story 1** (fast, non-blocking folder open). Validate, then add
  US2 (concurrent thumbnails + progressive previews), then US3 (queries + change-driven regeneration).
- **Incremental**: the `PhotoFolder` class consolidates folder logic first (US1); the producer–consumer
  pipeline and progressive UI land in US2; queries and live updates in US3. Per-photo work stays in `photo`.
