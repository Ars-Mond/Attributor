---
description: "Task list for Batch Metadata Save & Unified Event Contract"
---

# Tasks: Batch Metadata Save & Unified Event Contract

**Input**: Design documents from `specs/004-batch-metadata-save/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: Included — the project has an integration-test suite and `quickstart.md` enumerates
batch + contract validation scenarios. Backend tests use the `image` crate to generate fixtures.

**Organization**: Grouped by user story — US1 (fast multithreaded save), US2 (per-file status,
progress, cancel), US3 (unified typed event contract). The batch lives in a new `batch` module;
the event contract lives in a new `events.rs` (names + payload types only; emission stays in the
owning domains). Concurrency uses `rayon` inside `spawn_blocking`; progress streams over
`tauri::ipc::Channel` (§VIII, §IX, constitution v1.1.0).

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different file, no dependency on an incomplete task)
- **[Story]**: US1 / US2 / US3 (user-story phases only)

## Path Conventions

Backend Rust under `src-tauri/src/`; integration tests under `src-tauri/tests/`; frontend under `src/lib/`.

---

## Phase 1: Setup (Shared Infrastructure)

- [ ] T001 Add `rayon = "1"` to `[dependencies]` in `src-tauri/Cargo.toml` (batch parallelism per Constitution §VIII; justified in plan Complexity Tracking)
- [ ] T002 Create the batch module skeleton `src-tauri/src/batch/mod.rs` and declare `pub mod batch;` in `src-tauri/src/lib.rs`
- [ ] T003 Create the event-contract module skeleton `src-tauri/src/events.rs` and declare `pub mod events;` in `src-tauri/src/lib.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared types/state and the reusable single-file write path that every story builds on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [ ] T004 [P] Define the channel payload types in `src-tauri/src/events.rs`: `BatchProgress { index, status }` and `ItemStatus` enum (`Ok { path }` | `Failed { error }` | `Cancelled`), both `#[derive(Serialize, Clone)]` + `#[serde(rename_all = "camelCase")]` (tagged enum)
- [ ] T005 [P] Define `BatchState { cancel: Mutex<Option<Arc<AtomicBool>>> }` in `src-tauri/src/batch/mod.rs` and register `.manage(BatchState::default())` in `run()` in `src-tauri/src/lib.rs`
- [ ] T006 Extract `save_one(item: SaveRequest) -> Result<String, String>` (rename-if-changed + `photo::write_metadata`) from `save_metadata` into `src-tauri/src/batch/mod.rs`, and refactor `save_metadata` in `src-tauri/src/lib.rs` to delegate to it (guarantees FR-010)

**Checkpoint**: Types, managed cancel state, and the shared per-file write path exist.

---

## Phase 3: User Story 1 - Save many photos at once, fast (Priority: P1) 🎯 MVP

**Goal**: One batched backend call writes the whole selection concurrently with `rayon`, far faster than the sequential loop.

**Independent Test**: Select N photos, apply shared fields, save once → every file has the new metadata (identical to single-file save) and wall-clock is far below the one-by-one flow.

- [ ] T007 [US1] Implement `save_metadata_batch(items: Vec<SaveRequest>, on_progress: tauri::ipc::Channel<BatchProgress>, state: State<BatchState>) -> Result<Vec<ItemStatus>, String>` in `src-tauri/src/batch/mod.rs`: install a fresh cancel flag (swap-out-old idiom), `tokio::task::spawn_blocking` + `rayon` `into_par_iter().enumerate()`, per item call `save_one` → `ItemStatus`, `on_progress.send(BatchProgress { index, status })`, collect the ordered `Vec<ItemStatus>`
- [ ] T008 [US1] Register `save_metadata_batch` in `tauri::generate_handler![…]` in `src-tauri/src/lib.rs`
- [ ] T009 [US1] Replace the sequential loop in `handleBatchSave` (`src/lib/panel/MetadataPanel.svelte`) with a single `invoke("save_metadata_batch", { items, onProgress: ch })` using `new Channel` (`@tauri-apps/api/core`); build `items` from data already loaded by `loadBatchData` (no per-file re-read, FR-006/FR-007); add a temporary hand-written `BatchProgress`/`ItemStatus` TS type in `src/lib/types.ts` (replaced by the generated type in US3)
- [ ] T010 [P] [US1] Test: concurrent batch correctness in `src-tauri/tests/batch_test.rs` — write N generated images via `save_metadata_batch`'s path and assert each file reads back the expected metadata, identical to a single-file save

**Checkpoint**: Batch saving is concurrent and correct; MVP demonstrable.

---

## Phase 4: User Story 2 - Per-file outcome, live progress, cancel (Priority: P2)

**Goal**: The user sees incremental progress and per-file outcomes (saved/failed/cancelled), failures don't abort the batch, and a running batch can be cancelled.

**Independent Test**: Run a mixed batch (one unwritable file) → the rest save, the bad file reports failed; cancel mid-run → not-yet-started files report cancelled and every item has an outcome.

- [ ] T011 [US2] Implement `cancel_batch(state: State<BatchState>)` (sync) in `src-tauri/src/batch/mod.rs`, register it in `generate_handler!` (`src-tauri/src/lib.rs`), and add the per-item cancel check in `save_metadata_batch` so not-yet-started items resolve to `ItemStatus::Cancelled` (FR-017/FR-018)
- [ ] T012 [US2] Add progress + per-file status UI in `src/lib/panel/MetadataPanel.svelte` using `$state` (done/total derived from messages keyed by `index`), a Cancel button calling `invoke("cancel_batch")`, and best-effort result display; reuse existing button primitives and SCSS tokens (§III/§V)
- [ ] T013 [US2] Guard against watcher churn during a batch in `src/lib/panel/FilesPanel.svelte`: while a batch is in progress, skip the `folder-changed`-driven rescan so a metadata-only batch triggers no full rescan / thumbnail-pipeline restart (FR-008)
- [ ] T014 [P] [US2] Test: best-effort + cancellation in `src-tauri/tests/batch_test.rs` — a batch with one unwritable item saves the others and reports the failure (no panic); setting the cancel flag mid-run yields a mix of `ok`/`cancelled` with every item accounted for

**Checkpoint**: Batch is observable, resilient to per-file failure, and cancellable; US1 + US2 work together.

---

## Phase 5: User Story 3 - Unified typed event contract (Priority: P3)

**Goal**: Event names + payloads live once in `events.rs`; `ts-rs` generates a checked-in TS type guarded by a drift test; existing broadcasts migrate onto it with no behavior change.

**Independent Test**: `folder-changed`/`thumbnail-ready` still drive the tree; changing a payload without regenerating fails the contract test (and a stale frontend type fails `svelte-check`).

- [ ] T015 [US3] Add `ts-rs` (pinned major version) to `[dev-dependencies]` in `src-tauri/Cargo.toml`
- [ ] T016 [US3] In `src-tauri/src/events.rs`: add name constants `FOLDER_CHANGED`/`THUMBNAIL_READY` and payload structs `FolderChanged { path }` / `ThumbnailReady { path }`, and add `#[cfg_attr(test, derive(ts_rs::TS))]` + `#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/events.d.ts"))]` to all contract types (`FolderChanged`, `ThumbnailReady`, `BatchProgress`, `ItemStatus`)
- [ ] T017 [US3] Migrate emitters: `src-tauri/src/folder/pipeline.rs` uses `events::ThumbnailReady` + `events::THUMBNAIL_READY` (remove its local struct); `src-tauri/src/folder/watch.rs` emits `events::FOLDER_CHANGED` with `FolderChanged { path }` (promote from the bare `String` payload) — no observable behavior change (FR-015)
- [ ] T018 [US3] Generate and commit `src/lib/generated/events.d.ts` (run the contract test once), and add `src/lib/events.ts` with an `EVENT` name catalog and a typed `listenEvent` wrapper over `@tauri-apps/api/event`
- [ ] T019 [US3] Migrate frontend listeners in `src/lib/panel/FilesPanel.svelte` to `listenEvent<ThumbnailReady|FolderChanged>(EVENT.…)`, and replace the US1 hand-written batch TS type with the generated `BatchProgress`/`ItemStatus` from `src/lib/generated/events.d.ts`
- [ ] T020 [P] [US3] Test: drift guard in `src-tauri/tests/events_contract_test.rs` — re-export the contract types to a temp dir and assert byte-equality (line endings normalized for §IV) with the committed `src/lib/generated/events.d.ts` (FR-016)

**Checkpoint**: All three user stories independently functional; event contract is drift-proof.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [ ] T021 [P] Logging audit across `src-tauri/src/batch/` and `src-tauri/src/events.rs` — per-file write failures and `Channel::send` errors logged in concise English; no `println!`/`dbg!` (§VI)
- [ ] T022 [P] Run `npx svelte-check --tsconfig ./tsconfig.json` for the changed frontend files and resolve any issues
- [ ] T023 Run `cargo test` (all green) and validate the `quickstart.md` scenarios S1–S5

---

## Dependencies & Execution Order

- **Setup (T001–T003)** → **Foundational (T004–T006)** → user stories.
- **US1 (T007–T010)** depends on Foundational (`save_one` T006, `BatchState` T005, `BatchProgress`/`ItemStatus` T004); T008 depends on T007; T009 depends on T007.
- **US2 (T011–T014)** depends on US1 (extends `save_metadata_batch` and its UI); T011 → builds on T007 (same file); T012 → builds on T009 (same file).
- **US3 (T015–T020)** depends on Foundational (`events.rs`); independent of US1/US2 except T019 (replaces the US1 hand-written TS type) and T016 (adds ts-rs attrs to the US1-defined `BatchProgress`/`ItemStatus`).
- **Polish (T021–T023)** last; T023 after all implementation.
- Same-file sequencing: `batch/mod.rs` T005 → T006 → T007 → T011; `events.rs` T004 → T016; `MetadataPanel.svelte` T009 → T012; `FilesPanel.svelte` T013 → T019.

## Parallel Opportunities

- Foundational: T004 (events.rs) ∥ T005 (batch/mod.rs) — different files.
- Each story's test runs parallel to that story's implementation (different files): T010 ∥ T007–T009; T014 ∥ T011–T013; T020 ∥ T015–T019.
- Polish: T021 (Rust) ∥ T022 (frontend).
- Note: test tasks T010/T014 share `batch_test.rs` → sequential among themselves.

## Parallel Example: User Story 1

```bash
# Implementation and its test touch different files → run together:
Task: "T007 Implement save_metadata_batch in src-tauri/src/batch/mod.rs"
Task: "T010 Concurrent batch correctness test in src-tauri/tests/batch_test.rs"
```

## Implementation Strategy

- **MVP** = Setup + Foundational + **User Story 1** (one concurrent batched save). Validate, then add
  US2 (status/progress/cancel), then US3 (typed event contract + migration).
- **Incremental**: US1 ships a working fast batch (with a temporary hand-written TS type); US2 makes
  it observable and cancellable; US3 hardens the event contract and replaces the temporary type with
  the generated, drift-guarded one. Per-photo write internals stay in `photo` (single responsibility).
