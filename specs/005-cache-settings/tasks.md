---
description: "Task list for Configurable Photo Caching"
---

# Tasks: Configurable Photo Caching

**Input**: Design documents from `specs/005-cache-settings/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: Included — `quickstart.md` enumerates validation scenarios and the plan lists split-generation
and scoped-pipeline tests. Backend tests use the `image` crate for fixtures.

**Organization**: Grouped by user story — US1 (photo caching / viewer), US2 (small-thumbnail caching /
list), US3 (lazy generation), US4 (current-folder-only scope). No new dependencies. The shared backend
split (`ensure(low, high)`), the parameterized pipeline + commands (`GenConfig`, `cache_thumbnail`), and
the four settings are foundational; each story is a thin display/trigger layer plus its tests.

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different file, no dependency on an incomplete task)
- **[Story]**: US1 / US2 / US3 / US4 (user-story phases only)

## Path Conventions

Backend Rust under `src-tauri/src/`; integration tests under `src-tauri/tests/`; frontend under `src/lib/` and `src/routes/`.

---

## Phase 1: Setup (Shared Infrastructure)

- [x] T001 Register a "Caching" settings section and four boolean settings — `cache.photo` (default false), `cache.smallThumbnails` (default false), `cache.lazy` (default false), `cache.currentFolderOnly` (default true) — with labels/descriptions in `src/lib/settings/index.ts` (the registry auto-renders boolean fields)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The per-size generation split and the parameterized pipeline/commands that every story builds on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [x] T002 Refactor `photo::thumbnail` to `pub fn ensure(source: &Path, low: bool, high: bool) -> Result<Thumbnails, String>` — reuse each valid cached size, decode the source once iff a requested size is missing, generate only requested missing sizes — and make `ensure_thumbnails(source) = ensure(source, true, true)`, in `src-tauri/src/photo/thumbnail.rs` (keep the `photo/mod.rs` re-export)
- [x] T003 Add `GenConfig { low, high, recursive }` (`#[serde(rename_all = "camelCase")]`) and parameterize `pipeline::start(app, root, cancel, low, high, recursive)` — `collect_jobs` walks only the top level when `!recursive`, the whole subtree when `recursive`; each worker calls `ensure(low, high)`, in `src-tauri/src/folder/pipeline.rs`
- [x] T004 Make `PhotoFolder::open` / `rescan` accept a `GenConfig` and start the pipeline only when `low || high` (otherwise scan the tree and generate nothing), in `src-tauri/src/folder/mod.rs`
- [x] T005 Update `open_folder` / `open_folder_path` / `scan_folder` to accept `gen: GenConfig`, add `cache_thumbnail(path: String, low: bool, high: bool) -> Result<Thumbnails, String>` (`spawn_blocking(|| ensure(..))`), and register `cache_thumbnail` in `generate_handler!` (keep `get_thumbnails` until US1 migrates the viewer), in `src-tauri/src/lib.rs`
- [x] T006 Derive `GenConfig` from the four settings (`low = !lazy && smallThumbnails`, `high = !lazy && photoCaching`, `recursive = !currentFolderOnly`) and pass it on the `open_folder` / `open_folder_path` / `scan_folder` invokes in `src/lib/panel/FilesPanel.svelte`

**Checkpoint**: Generation is per-size and config-driven; the app builds and opens folders per settings.

---

## Phase 3: User Story 1 - Photo caching on/off (Priority: P1) 🎯 MVP

**Goal**: The viewer shows the photo via a cached high thumbnail when "Photo caching" is on, and from the original directly when off.

**Independent Test**: Toggle "Photo caching" off → opening a photo shows the original and creates no high thumbnail; toggle on → the viewer shows a cached high thumbnail.

- [x] T007 [US1] In `src/routes/+page.svelte` `showInViewer`: when `cache.photo` is on, show the high via `invoke('cache_thumbnail', {path, low: false, high: true})` (fallback to the original until ready / on error); when off, set `imageSrc = convertFileSrc(path)` directly with no generation. Remove the now-unused `get_thumbnails` command and its `generate_handler!` registration in `src-tauri/src/lib.rs`
- [x] T008 [P] [US1] Test: `ensure(path, false, true)` writes only the high thumbnail (low absent) and reuses a valid high without regenerating, in `src-tauri/tests/thumbnail_test.rs`

**Checkpoint**: Viewer display follows "Photo caching"; MVP demonstrable.

---

## Phase 4: User Story 2 - Cache small thumbnails independently (Priority: P2)

**Goal**: The list/tree shows cached small previews when "Cache small thumbnails" is on, and originals directly when off — independently of photo caching.

**Independent Test**: With "Cache small thumbnails" on (photo caching off), the list shows cached small previews while the viewer still shows originals.

- [x] T009 [US2] Gate the small preview on `cache.smallThumbnails` in `src/lib/reusable/FileTree.svelte` — on → show `thumb_low` when `readyThumbs` has the path (placeholder until ready); off → show the original directly (`convertFileSrc(node.path)`)
- [x] T010 [US2] Apply the same `cache.smallThumbnails` gating to the icons mode in `src/lib/panel/FilesPanel.svelte` (cached low when ready vs original direct)
- [x] T011 [P] [US2] Test: `ensure(path, true, false)` writes only the low thumbnail (high absent), and `ensure(path, true, true)` decodes once and writes both, in `src-tauri/tests/thumbnail_test.rs`

**Checkpoint**: List display follows "Cache small thumbnails", independent of US1.

---

## Phase 5: User Story 3 - Lazy generation on display (Priority: P3)

**Goal**: With "Lazy caching" on, nothing is generated at folder open; the small thumbnail is generated when its list item is shown, and the large when the photo is opened in the viewer.

**Independent Test**: Lazy on → opening a folder generates 0 thumbnails; showing a list item generates its small; opening a photo generates its large.

- [x] T012 [US3] Lazy low trigger in `src/lib/reusable/FileTree.svelte` (and icons mode in `src/lib/panel/FilesPanel.svelte`): when `cache.smallThumbnails && cache.lazy` and an image item is shown but its low isn't ready, invoke `cache_thumbnail(path, true, false)` once and add `path` to `readyThumbs` on success
- [x] T013 [US3] Verify lazy disables eager generation: with `cache.lazy` on, the `GenConfig` derivation (T006) yields `low = high = false` so the pipeline is not started, while the viewer still generates the high on open (T007), in `src/lib/panel/FilesPanel.svelte`
- [x] T014 [P] [US3] Test: `pipeline::start` with `low = false, high = false` performs no work (no jobs enqueued), in `src-tauri/tests/folder_test.rs`

**Checkpoint**: Generation timing follows "Lazy caching"; US1 + US2 + US3 work together.

---

## Phase 6: User Story 4 - Current folder only (Priority: P2)

**Goal**: Automatic generation covers only the opened folder's top level when "Current folder only" is on; an explicit viewer-open is exempt.

**Independent Test**: Current-folder-only on (default) → opening a folder with subfolders auto-caches only top-level photos; opening a subfolder photo in the viewer still caches its large thumbnail.

- [x] T015 [US4] Verify `recursive = !cache.currentFolderOnly` flows from settings through `GenConfig` (T006) to `pipeline::start`, and that the explicit viewer-open path (`cache_thumbnail`, T007) is unaffected by scope (FR-017), in `src/lib/panel/FilesPanel.svelte`
- [x] T016 [P] [US4] Test: `pipeline::start(..., recursive = false)` enqueues only top-level photos while `recursive = true` enqueues subfolder photos too, in `src-tauri/tests/folder_test.rs`

**Checkpoint**: Generation scope follows "Current folder only"; all four stories independently functional.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [x] T017 [P] Cleanup/logging audit across `src-tauri/src/photo/thumbnail.rs`, `src-tauri/src/folder/`, and `src-tauri/src/lib.rs` — `ensure`/`cache_thumbnail` error paths log in concise English, no leftover `get_thumbnails`, no `println!`/`dbg!` (§VI)
- [x] T018 [P] Run `npx svelte-check --tsconfig ./tsconfig.json` for the changed frontend files and resolve any issues
- [x] T019 Run `cargo test` (all green) and validate the `quickstart.md` scenarios S1–S7

---

## Dependencies & Execution Order

- **Setup (T001)** → **Foundational (T002–T006)** → user stories.
- Foundational chain: T002 → T003 → T004 → T005 → T006 (each builds on the prior; backend signature changes cascade to the frontend plumbing in T006).
- **US1 (T007–T008)** depends on T005/T006 (`cache_thumbnail`).
- **US2 (T009–T011)** depends on T002 (`ensure` low) and T006 (eager `GenConfig.low`).
- **US3 (T012–T014)** depends on T005/T006 (`cache_thumbnail`, lazy-aware derivation).
- **US4 (T015–T016)** depends on T003/T006 (`recursive`).
- **Polish (T017–T019)** last; T019 after all implementation.
- Same-file sequencing: `lib.rs` T005 → T007; `FilesPanel.svelte` T006 → T010 → T012 → T013 → T015; `FileTree.svelte` T009 → T012; `thumbnail.rs` T002 before its tests T008/T011.

## Parallel Opportunities

- Each story's test runs parallel to that story's implementation (different files): T008 ∥ T007; T011 ∥ T009–T010; T014 ∥ T012–T013; T016 ∥ T015.
- Test tasks T008/T011 share `thumbnail_test.rs` (sequential among themselves); T014/T016 share `folder_test.rs` (sequential among themselves).
- Polish: T017 (Rust) ∥ T018 (frontend).

## Parallel Example: User Story 1

```bash
# Implementation and its test touch different files → run together:
Task: "T007 Viewer photo-caching gating in src/routes/+page.svelte"
Task: "T008 ensure(false,true) high-only test in src-tauri/tests/thumbnail_test.rs"
```

## Implementation Strategy

- **MVP** = Setup + Foundational + **User Story 1** (viewer honors Photo caching). Validate, then add US2
  (list small caching), US3 (lazy timing), US4 (current-folder scope) incrementally.
- **Incremental**: the backend split + parameterized generation land once (foundational); each story is a
  small display/trigger layer over it. The thumbnail cache location/format is unchanged.
