---
description: "Task list for Photo Thumbnail Cache"
---

# Tasks: Photo Thumbnail Cache

**Input**: Design documents from `specs/002-thumbnail-cache/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: Included — the project has an integration-test suite and `quickstart.md` enumerates
thumbnail validation scenarios. Test inputs (landscape/portrait, PNG/WebP) are generated in-test
via the `image` crate; existing fixtures under `src-tauri/test_images/` are also used.

**Organization**: Grouped by user story — US1 (viewer high preview), US2 (tree low previews),
US3 (cache reuse & locality). The generation engine is shared, so it is Foundational.

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different file, no dependency on an incomplete task)
- **[Story]**: US1 / US2 / US3 (user-story phases only)

## Path Conventions

Backend Rust under `src-tauri/src/`; integration tests under `src-tauri/tests/`; frontend under
`src/lib/` and `src/routes/`. Builds on the existing `photo` module and the `image` crate (no new deps).

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Scaffold the thumbnail module.

- [ ] T001 Create `src-tauri/src/photo/thumbnail.rs` (constants `LOW_MAX=360`, `HIGH_MAX=1920`, `JPEG_QUALITY=75`; `Thumbnails { low, high }` with `#[serde(rename_all = "camelCase")]`; `ensure_thumbnails(&Path)` stub) and declare `mod thumbnail;` + re-export `ensure_thumbnails`/`Thumbnails` in `src-tauri/src/photo/mod.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The generation engine, the command, frontend access, and tree hygiene — shared by all stories.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [ ] T002 Implement `generate(src, dst, max)` — longest-side resize (Lanczos3, no upscale), convert to `rgb8`, encode JPG at `JPEG_QUALITY`, write — in `src-tauri/src/photo/thumbnail.rs`
- [ ] T003 Implement path derivation (`<dir>/_thumbnail/<file_name>.<low|high>.jpg`), `_thumbnail` folder creation, `is_valid`, and `ensure_thumbnails` (reuse valid / else generate both) in `src-tauri/src/photo/thumbnail.rs`
- [ ] T004 Add async command `get_thumbnails(path) -> Result<Thumbnails, String>` (CPU work in `tokio::task::spawn_blocking`) and register it in the `invoke_handler` in `src-tauri/src/lib.rs`
- [ ] T005 [P] Add the frontend in-memory cache helper `getThumbnails(path)` (runes map `path → {low, high}`, dedupes concurrent calls, invokes `get_thumbnails`) in `src/lib/panel/thumbnailCache.svelte.ts`
- [ ] T006 [P] Exclude child directories named `_thumbnail` from the scan in `src-tauri/src/filetree.rs` (`scan_dir`)

**Checkpoint**: `get_thumbnails` returns valid low/high paths; `_thumbnail` is hidden from the tree.

---

## Phase 3: User Story 1 - Large preview in the viewer (Priority: P1) 🎯 MVP

**Goal**: Opening a photo shows a 1920px (longest side) preview from cache, with a loading indicator on first generation.

**Independent Test**: Open an uncached photo → loading indicator, then the high preview; a high thumbnail file appears in `_thumbnail`. Reopen → instant, no regeneration.

- [ ] T007 [P] [US1] Add high-variant tests (longest side = 1920, no upscale when source shorter, valid JPG output) in `src-tauri/tests/thumbnail_test.rs`
- [ ] T008 [US1] Add a `loading` state and a loading indicator (styled with `_mixins`/`_themes` tokens) to `src/lib/panel/ImageViewerPanel.svelte`
- [ ] T009 [US1] Derive the viewer source from `getThumbnails(activePath).high` (loading until resolved, graceful fallback on error) in `src/routes/+page.svelte`

**Checkpoint**: The viewer shows cached high previews with a first-open loading indicator.

---

## Phase 4: User Story 2 - Small previews in the file hierarchy (Priority: P2)

**Goal**: The file tree shows 360px (longest side) previews from cache instead of the full original.

**Independent Test**: Show a folder in content view → each photo shows a small preview served from `_thumbnail`; re-display reuses them.

- [ ] T010 [P] [US2] Add low-variant tests (longest side = 360, `_thumbnail` folder created) in `src-tauri/tests/thumbnail_test.rs`
- [ ] T011 [US2] In content mode, render the `low` thumbnail via `getThumbnails(node.path)` (placeholder until resolved) instead of `convertFileSrc(node.path)` in `src/lib/reusable/FileTree.svelte`

**Checkpoint**: Browsing in content view shows lightweight low previews; US1 and US2 both work.

---

## Phase 5: User Story 3 - Cache reuse and locality (Priority: P3)

**Goal**: Valid thumbnails are reused across sessions; invalid ones regenerate; `_thumbnail` stays out of the tree.

**Independent Test**: Generate, restart, reopen → no new files written; replace a thumbnail with a 0-byte file → it regenerates; a `_thumbnail` folder never appears in the tree.

- [ ] T012 [P] [US3] Add reuse (2nd call regenerates nothing), regenerate-on-invalid, and `_thumbnail`-excluded-from-`FileNode` tests in `src-tauri/tests/thumbnail_test.rs`
- [ ] T013 [US3] Harden `is_valid` to reject empty/undecodable thumbnail files so invalid ones regenerate (FR-011) in `src-tauri/src/photo/thumbnail.rs`

**Checkpoint**: All three user stories are independently functional.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Quality gates and tuning.

- [ ] T014 [P] Logging audit in `src-tauri/src/photo/thumbnail.rs` — error-site logging, concise English, no `println!`/`dbg!`
- [ ] T015 [P] Run `npx svelte-check --tsconfig ./tsconfig.json` for the changed frontend files and resolve any issues
- [ ] T016 Run `cargo test` (all green), validate the `quickstart.md` scenarios, and tune `JPEG_QUALITY` to meet SC-003 (low < 50 KB, high < 500 KB)

---

## Dependencies & Execution Order

- **Setup (T001)** → **Foundational (T002–T006)** → user stories.
- Within `thumbnail.rs`: T002 → T003 → T013 (same file, sequential). T004 (command) depends on T003.
- **US1 (T007–T009)** and **US2 (T010–T011)** depend on Foundational; they touch different files (viewer/page vs tree) and can proceed in parallel.
- **US3 (T012–T013)** depends on Foundational; T013 edits `thumbnail.rs` (after T003).
- **Polish (T014–T016)** last; T016 after all implementation.
- `get_thumbnails` (T004) underpins T005, T009, T011.

## Parallel Opportunities

- Foundational: T005 (frontend cache) and T006 (filetree exclusion) run parallel to the backend `thumbnail.rs` work (different files).
- Each story's test task is parallel to that story's implementation (different files): T007 ∥ T008–T009; T010 ∥ T011; T012 ∥ T013.
- US1 and US2 implementation can run concurrently.
- Polish: T014 (Rust) ∥ T015 (frontend).

```text
# Foundational parallel batch (different files):
Task: "T005 frontend thumbnailCache.svelte.ts"
Task: "T006 exclude _thumbnail in filetree.rs scan_dir"
```

## Implementation Strategy

- **MVP** = Setup + Foundational + **User Story 1** (viewer high preview). Ship/validate, then add US2 (tree), then US3 (reuse hardening).
- **Incremental**: each story is an independently testable increment; the shared engine lands in Foundational so US1/US2 only wire UI.
