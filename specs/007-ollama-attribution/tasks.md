---

description: "Task list for Ollama vision auto-attribution"
---

# Tasks: Ollama Vision Auto-Attribution

**Input**: Design documents from `/specs/007-ollama-attribution/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: No vitest/playwright tasks (out of scope per prior project decision). Automated guards are
`cargo test` (incl. the `events_contract` ts-rs drift test for the new `PullProgress`) and `npm run check`
(svelte-check + i18n catalog completeness). Manual validation per quickstart.md.

**Deferred content**: Default prompts, the offered-model list, and default run parameters are NOT authored
here — only their holding structures. They will be supplied by the user before/at implementation.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: parallelizable (different files, no dependency on an incomplete task)
- **[Story]**: US1..US5 (setup/foundational/polish carry no story label)

## Path Conventions

Backend `src-tauri/src/` (new module `ollama/`), frontend `src/lib/` + `src/routes/`.

---

## Phase 1: Setup

- [ ] T001 [P] Add `reqwest = {version="0.12", default-features=false, features=["rustls-tls","json","stream"]}`, `base64 = "0.22"`, and `futures-util = "0.3"` to `[dependencies]` in `src-tauri/Cargo.toml` (research.md Decision 2).
- [ ] T002 [P] Confirm a clean baseline: `npm run check` and `cargo check` (in `src-tauri/`) both pass before changes.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The Ollama engine (client + commands), the reusable progress overlay, and the settings
scaffolding that every story builds on.

**⚠️ CRITICAL**: No user story can begin until this phase is complete.

- [ ] T003 [P] Define backend types in `src-tauri/src/ollama/types.rs`: `OllamaStatus{reachable,installed,version}`, `OllamaModel{name,size}`, `AttributionConfig{base_url,model,prompt,think,keep_alive,options,format}`, `AttributionResult{title,description,keywords,categories}`, and `Think` (untagged enum `Bool(bool)|Level(String)`). All `#[serde(rename_all="camelCase")]` (contracts/tauri-commands.md).
- [ ] T004 Implement the Ollama HTTP client in `src-tauri/src/ollama/client.rs` using `reqwest` (rustls): `version(base)`, `list_tags(base)`, `pull(base, model, on_line)` streaming NDJSON via `bytes_stream()`+`futures-util`, and `generate(base, req)` for one non-stream vision inference (base64 image, `format` schema, `options`, `think`). Generous read timeout for inference, short connect timeout for the heartbeat; never panic (contracts/ollama-http.md) (depends on T003).
- [ ] T005 Add `PullProgress{status,digest:Option,total:Option,completed:Option}` to `src-tauri/src/events.rs` (serde camelCase + `cfg_attr(test, derive(ts_rs::TS))`, added to the `events_contract` render list) and regenerate `src/lib/generated/events.d.ts` (`UPDATE_EVENTS_CONTRACT=1 cargo test events_contract`). Reuse existing `BatchProgress`/`ItemStatus` for batch attribution.
- [ ] T006 Implement `src-tauri/src/ollama/mod.rs`: `OllamaState` (managed `Arc<AtomicBool>` cancel flag, swap pattern from `batch/mod.rs`) and the shared commands `ollama_status`, `ollama_list_models`, `ollama_pull_model` (`tauri::ipc::Channel<PullProgress>`, cancelable), `open_ollama_download` (via `tauri-plugin-opener` → `https://ollama.com/download`), and `ollama_cancel`. All `Result<T,String>` (depends on T004, T005).
- [ ] T007 Register the new commands in `tauri::generate_handler![…]` and add `.manage(OllamaState::default())` in `src-tauri/src/lib.rs`; declare `mod ollama;` (depends on T006).
- [ ] T008 [P] Frontend client glue in `src/lib/ollama/ollama.ts`: typed invoke wrappers `ollamaStatus`, `listModels`, `pullModel(onProgress)`, `openDownload`, `cancelOllama`, and an `attributionConfig()` helper that assembles `AttributionConfig` from settings (`ollama.baseUrl`, `ollama.activeModel`, `ollama.responseFormat` parsed to an object, and a prompt/params placeholder until US5).
- [ ] T009 [P] Implement the reusable progress store in `src/lib/progress.svelte.ts`: `$state`-backed singleton with `active()`, `run({label,total?,blocking?,cancelable?,onCancel?}) -> {update,done}` (rejects if one is already active) (contracts/frontend-ui.md).
- [ ] T010 Create `src/lib/reusable/ProgressOverlay.svelte`: a top-most overlay at `z-index: 600` (above SettingsDialog 500) driven by `progress.svelte.ts`; shows label, determinate/indeterminate progress, and a Cancel button; `blocking` covers the screen and swallows pointer events; uses SCSS tokens (depends on T009).
- [ ] T011 Mount `<ProgressOverlay />` once in `src/routes/+page.svelte` (depends on T010).
- [ ] T012 In `src/lib/settings/SettingsDialog.svelte`, render a section's custom component **instead of** its descriptor fields when `activeSection.component` exists (component XOR fields) so custom pages fully own their UI (no regression: ShortcutsPage has no fields).
- [ ] T013 Create stub components `src/lib/settings/OllamaSettingsPage.svelte` and `OllamaModelsPage.svelte` (placeholder markup), then in `src/lib/settings/index.ts` register the `ollama` (order 4) and `ollama-models` (order 5) sections with those components and register the persisted keys `ollama.baseUrl` (default `http://localhost:11434`), `ollama.activeModel` (default `''`), `ollama.responseFormat` (default = the fixed JSON schema text), `ollama.modelProfiles` (custom, default `[]`) (depends on T012).
- [ ] T014 [P] Add the shared i18n keys to `src/lib/i18n/types.ts`, `en.ts`, `ru.ts`: section labels (`settings.section.ollama`, `settings.section.ollamaModels`), and common `ollama.*` strings (attribution in-progress/complete/failed, `ollama.unavailable.tooltip`, pull/progress/cancel labels).

**Checkpoint**: backend engine + commands compile and register; overlay mounts; settings show empty Ollama
sections; `npm run check` and `cargo test` pass.

---

## Phase 3: User Story 1 - Auto-attribute a single photo (Priority: P1) 🎯 MVP

**Goal**: The metadata panel's "Attribute via Ollama" button analyses the open photo and fills the form
(overwrite title/description/categories, append+dedupe keywords); the user reviews and saves.

**Independent Test**: With Ollama reachable and a model selected, open a photo → click attribute → fields
fill from the image; nothing saved until the user saves; button greys out with a tooltip when unavailable.

- [ ] T015 [US1] Implement single inference in `src-tauri/src/ollama/attribute.rs`: `attribute_one(path, config)` — read the image (optionally downscale via `image`), base64-encode, build the `/api/generate` request, call `client::generate`, parse the `response` string as JSON, validate the required fields, and return `AttributionResult` (title/description/keywords/categories; the three flags validated-then-ignored). On any failure return `Err(String)` and touch nothing (FR-015) (depends on T004).
- [ ] T016 [US1] Add the `attribute_photo(path, config) -> Result<AttributionResult,String>` command in `src-tauri/src/ollama/mod.rs` and register it in `src-tauri/src/lib.rs` (depends on T015, T007).
- [ ] T017 [US1] Add `attributePhoto(path, config)` to `src/lib/ollama/ollama.ts` (depends on T008).
- [ ] T018 [US1] Add a reactive availability gate in `src/lib/ollama/ollama.ts` (or a small `ollama.svelte.ts`): poll/cache `ollamaStatus` + read `ollama.activeModel` → `available` derived used to enable/disable the attribute button (FR-007/009) (depends on T008).
- [ ] T019 [US1] In `src/lib/panel/MetadataPanel.svelte` add the single-mode **Attribute via Ollama** button: disabled+greyed with `title={t('ollama.unavailable.tooltip')}` when not available; on click run `attributePhoto` under an indeterminate `progress.run({cancelable:true,onCancel:cancelOllama})`, then apply the result — `title`/`description`/`categories` (`result.categories.join(', ')`) overwrite, keywords appended via the existing `addKeyword` loop (dedupe), mark dirty; add its i18n keys (depends on T017, T018, T011).

**Checkpoint**: single-photo attribution fills the form and respects availability; MVP demoable.

---

## Phase 4: User Story 2 - Configure and detect Ollama in settings (Priority: P1)

**Goal**: The Ollama settings category lets the user check/install Ollama, download an offered model, select
the active installed model, and inspect the debug JSON format.

**Independent Test**: Settings → Ollama: Check reports reachable/installed correctly; download shows progress
and the model then appears installed; selecting a model persists; the format field warns it is debug-only.

- [ ] T020 [US2] Flesh out `src/lib/settings/OllamaSettingsPage.svelte`: a status row + **Check** button (`ollamaStatus`); an **Install** button shown when not installed (`openDownload`); a **base URL** field bound to `ollama.baseUrl`; an **installed-models** dropdown (`listModels` → writes `ollama.activeModel`); an **offered-models** dropdown + **Download** button (`pullModel` streamed through `progress.run({blocking:true,cancelable:true})`, refresh installed list on success); a **JSON response-format** textarea bound to `ollama.responseFormat` with a `field-desc` debug-only warning. All strings via `t()`; add the `settings.ollama.*` i18n keys (depends on T013, T008, T011).
- [ ] T021 [US2] Add the offered-models list structure in `src/lib/ollama/models.ts` (an exported `OFFERED_MODELS: {id,label}[]`, **left empty/deferred** with a comment that the user supplies contents) and consume it in the offered-models dropdown (depends on T020).

**Checkpoint**: Ollama can be detected, installed (guided), a model downloaded and selected — enabling US1 end to end.

---

## Phase 5: User Story 3 - Batch attribute and always save (Priority: P2)

**Goal**: Attribute every selected photo sequentially and always save, with overlay progress + cancel.

**Independent Test**: Select N photos → batch attribute → each is attributed and saved automatically; a
failed item is recorded without aborting; cancel stops further processing and keeps saved files.

- [ ] T022 [US3] Implement `attribute_batch(paths, config, cancel, on_progress)` in `src-tauri/src/ollama/attribute.rs`: a SEQUENTIAL async loop — check `cancel`, `attribute_one`, read existing metadata (`photo::read_metadata`), merge (overwrite title/description/categories, append+dedupe keywords), save via the existing per-file write (`batch::save_one`), and emit one `BatchProgress{index,status}`; on item error emit `ItemStatus::Failed` and continue (FR-013/014) (depends on T015).
- [ ] T023 [US3] Add `attribute_batch(paths, config, on_progress: Channel<BatchProgress>, state) -> Result<Vec<ItemStatus>,String>` command in `src-tauri/src/ollama/mod.rs` (reusing `OllamaState` cancel) and register it in `src-tauri/src/lib.rs` (depends on T022, T007).
- [ ] T024 [US3] Add `attributeBatch(paths, config, onProgress)` to `src/lib/ollama/ollama.ts` (depends on T008).
- [ ] T025 [US3] In `src/lib/panel/MetadataPanel.svelte` add the batch-mode **Attribute via Ollama** action: run `attributeBatch` through `progress.run({blocking:true,cancelable:true,onCancel:cancelOllama,total:paths.length})`, update progress from the Channel, reload batch data on completion; add i18n keys (depends on T024, T011).

**Checkpoint**: batch attribution attributes + saves every photo with progress and cancel.

---

## Phase 6: User Story 4 - Reusable global progress overlay with cancel (Priority: P2)

**Goal**: The overlay is top-most and blocking, and the existing batch SAVE is routed through it with a freeze.

**Independent Test**: Any long op shows the overlay above all dialogs/menus with cancel; a normal batch save
now freezes the UI until done.

- [ ] T026 [US4] Route the existing `handleBatchSave` (`save_metadata_batch`) in `src/lib/panel/MetadataPanel.svelte` through `progress.run({blocking:true,cancelable:true,onCancel:()=>invoke('cancel_batch'),total})`, driving progress from the existing Channel and freezing the UI until done (FR-020) (depends on T011).
- [ ] T027 [US4] Finalize `src/lib/reusable/ProgressOverlay.svelte`: confirm `z-index:600` sits above SettingsDialog (500) and dialogs (200); blocking mode fully swallows pointer/keyboard; verify the cancel control and determinate/indeterminate states across pull/batch/single (depends on T010).

**Checkpoint**: one consistent top-most progress/cancel surface; batch save freezes correctly.

---

## Phase 7: User Story 5 - Manage per-model profiles (Priority: P3)

**Goal**: An "Ollama Models" category manages per-model profiles (id, run params, prompt) via list +
Create/Edit/Delete + Save/Cancel popup; the active model's profile drives attribution.

**Independent Test**: Create/edit/delete a profile (popup) → persists across restart; attribution uses the
profile matching the active model.

- [ ] T028 [US5] Flesh out `src/lib/settings/OllamaModelsPage.svelte`: a list of `ollama.modelProfiles` with **Create / Edit / Delete** buttons (modeled on the reference screenshot); selection drives Edit/Delete; persists to `ollama.modelProfiles`; add i18n keys (depends on T013).
- [ ] T029 [US5] Create `src/lib/settings/OllamaModelDialog.svelte`: a popup (existing dialog overlay pattern) to set model id (select from installed or free text), run parameters (context length, thinking mode, …), and the prompt, with **Save**/**Cancel**; add i18n keys (depends on T028).
- [ ] T030 [US5] Update `attributionConfig()` in `src/lib/ollama/ollama.ts` to use the profile whose `modelId === ollama.activeModel` (prompt/think/keepAlive/options); fall back to a built-in default (deferred) when none matches (depends on T017).

**Checkpoint**: profiles managed and persisted; attribution driven by the matching profile.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [ ] T031 [P] Run `npm run check` — svelte-check passes with all new i18n keys present in `en.ts`/`ru.ts` (completeness gate); fix gaps.
- [ ] T032 [P] Run `cargo check` and `cargo test` in `src-tauri/` — backend builds with the new crates and the `events_contract` test passes (regenerate if `PullProgress` changed).
- [ ] T033 Run quickstart.md Scenarios 1–6 manually and confirm SC-001..SC-008.
- [ ] T034 [P] Audit Constitution VI logging (Ollama calls/errors logged via `log`; frontend invoke failures via `@tauri-apps/plugin-log`) and confirm no `console.*` / `println!` / `dbg!` were introduced; verify the disabled/tooltip states.

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (P1)** → no deps.
- **Foundational (P2)** → after Setup; **blocks all stories**. Order: T003 → T004 → (T005) → T006 → T007; T008/T009 [P]; T010 → T011; T012 → T013; T014 [P].
- **US1 (P3)** → after Foundational. T015 → T016; T017/T018 → T019.
- **US2 (P4)** → after Foundational. T020 → T021. (US1 end-to-end needs US2 to have selected a model.)
- **US3 (P5)** → after US1 (reuses `attribute_one`). T022 → T023 → T024 → T025.
- **US4 (P6)** → after Foundational (overlay). T026, T027.
- **US5 (P7)** → after Foundational; T030 also relates to US1's config assembly.
- **Polish (P8)** → after the stories you intend to ship.

### Story independence

- **US1** needs the engine + a selected model (from US2). **US2** is independently testable (detect/install/
  download/select). **US3** reuses US1's single inference. **US4** needs only the foundational overlay.
  **US5** needs only foundational settings scaffolding.

### Shared-file notes (serialize edits)

- `src-tauri/src/lib.rs`: T007, T016, T023 (command registration) — sequential.
- `src-tauri/src/ollama/mod.rs`: T006, T016, T023 — sequential.
- `src-tauri/src/ollama/attribute.rs`: T015, T022 — sequential.
- `src/lib/ollama/ollama.ts`: T008, T017, T018, T024, T030 — sequential.
- `src/lib/panel/MetadataPanel.svelte`: T019, T025, T026 — sequential.
- `src/lib/i18n/{en,ru}.ts` + `types.ts`: T014 and each UI task's key additions — serialize.

## Parallel Opportunities

- Setup: T001 ∥ T002.
- Foundational: T003 ∥ T008 ∥ T009 ∥ T014 (different files); T010/T011 and T012/T013 are their own chains.
- Across stories after Foundational: US2 (T020) and US4 (T026/T027) and US5 (T028/T029) can proceed in
  parallel with the US1→US3 backend chain (different files).
- Polish: T031 ∥ T032 ∥ T034.

## Implementation Strategy

### MVP (US1 + US2)

1. Phase 1 Setup → Phase 2 Foundational (engine + overlay + settings scaffolding).
2. Phase 4 US2: configure/detect Ollama, download + select a model.
3. Phase 3 US1: attribute a single photo into the form.
4. **STOP & VALIDATE**: with Ollama running and a model selected, attribute one photo end to end.

### Incremental delivery

Foundational → US2 (config) → US1 (single) = MVP → US3 (batch) → US4 (overlay/freeze) → US5 (profiles) →
Polish. Default prompts / offered-model list / default params are dropped in (deferred content) before US1/US2
are truly usable.

## Notes

- `[P]` = different files, no incomplete-task dependency.
- Keep the backend pure-Rust (rustls), all IPC typed `Result<T,String>` with `Channel` streaming, no panics.
- Batch **attribution** is sequential (Ollama-bound); batch **save** stays rayon — do not parallelize HTTP.
- Every new user-facing string must exist in `en.ts` and `ru.ts` or `npm run check` fails.
- Commit per logical group; the implement phase ends with the mandatory phase commit (Constitution VII) and
  passing `npm run check` / `cargo test`.
