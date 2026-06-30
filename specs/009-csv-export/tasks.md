---

description: "Task list for CSV Export"
---

# Tasks: CSV Export

**Input**: Design documents from `/specs/009-csv-export/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/ipc-commands.md, quickstart.md

**Tests**: Rust unit tests are included **inline** (`#[cfg(test)]`) within the implementation tasks, matching this repo's convention (feature 008). The frontend gate is `npx svelte-check`. No separate test-first phase.

**Organization**: Tasks are grouped by user story (US1 → US2 → US3) so each story is an independently testable increment.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no incomplete dependencies)
- **[Story]**: US1 / US2 / US3 (Setup, Foundational, Polish have no story label)
- Exact file paths are included in each task

## Path Conventions

Desktop app (Tauri 2 + SvelteKit). Rust under `src-tauri/src/`, frontend under `src/lib/` and `src/routes/`, per plan.md.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create the new Rust module scaffolding so the rest compiles.

- [X] T001 Scaffold the Rust `csv` module: create `src-tauri/src/csv/mod.rs`, `src-tauri/src/csv/writer.rs`, `src-tauri/src/csv/cell.rs` (empty `pub` stubs with `mod writer; mod cell;` in `mod.rs`) and declare `mod csv;` in `src-tauri/src/lib.rs`; verify `cargo check` passes.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The shared CSV config types used by BOTH preset configuration (US1) and export (US2).

**⚠️ CRITICAL**: US1 and US2 both depend on this type layer.

- [X] T002 Define the shared CSV config TypeScript types + defaults in `src/lib/csv/csv.ts`: `AppValueType` (`'none' | 'fileName' | 'title' | 'description' | 'keywords' | 'category' | 'editorial' | 'matureContent' | 'illustration'`), `Delimiter` (`'comma' | 'semicolon' | 'tab'`), `BoolFormat` (`'yesNo' | 'trueFalse'`), `CsvField`, `CsvPreset`, `ExportSummary`, and `DEFAULT_CSV_PRESETS = []`. Per data-model.md.

**Checkpoint**: Type layer ready — US1 and US2 can begin.

---

## Phase 3: User Story 1 - Configure stock CSV presets (Priority: P1) 🎯 MVP

**Goal**: A new Settings → CSV category where the user creates/edits/deletes stock presets (name, unique file-name-safe identifier, delimiter) and builds an ordered, add/remove/reorder field list with conditional per-field inputs; presets persist.

**Independent Test**: Open Settings → CSV, create a preset with mixed-type fields, see the `none`-only default-value input and bool-only format control, reorder fields with arrows, hit duplicate/invalid identifier validation, restart, and confirm everything persisted (quickstart "Validate US1").

### Implementation for User Story 1

- [X] T003 [P] [US1] Add CSV settings i18n keys to `src/lib/i18n/types.ts`, `src/lib/i18n/en.ts`, `src/lib/i18n/ru.ts`: section label (`settings.section.csv`), preset fields (name, identifier, delimiter + comma/semicolon/tab option labels), field editor labels (csvColumn, valueType + the 9 value-type option labels, defaultValue, boolFormat + yes-no/true-false labels), actions (add field, remove, move up/down), and validation errors (`settings.csv.duplicateIdentifier`, `settings.csv.invalidIdentifier`, `settings.csv.nameRequired`, `settings.csv.fieldsRequired`).
- [X] T004 [P] [US1] Add helpers to `src/lib/csv/csv.ts`: `isBoolType(t)`/`isNoneType(t)`, `sanitizeIdentifier`/`isValidIdentifier` (file-name-safe per research R9 — reject `< > : " / \ | ? *`, control chars, leading/trailing dots-or-spaces, reserved Windows device names), and `createEmptyPreset()`/`createEmptyField()` factories (use `crypto.randomUUID()` for preset `id`).
- [X] T005 [US1] Create `src/lib/settings/CsvPresetDialog.svelte` — per-preset editor mirroring `OllamaModelDialog.svelte`: name input, identifier input (inline uniqueness + file-name-safe validation), delimiter `<select>`, and the field-list editor (add/remove field rows; per row: csvColumn input, valueType `<select>`, conditional defaultValue input shown only for `none` (FR-024), conditional boolFormat `<select>` shown only for bool types (FR-025), up/down arrow reorder via array `splice`). Reject saving a preset with zero fields (FR-036) with an inline error (`settings.csv.fieldsRequired`). Reuse `.md-input`/`.md-btn`/`.mp-row` SCSS recipes and `$fs-*`/`$radius-*` tokens; depends on T002/T004.
- [X] T006 [US1] Create `src/lib/settings/CsvPresetsPage.svelte` — preset list mirroring `OllamaModelsPage.svelte`: create/edit/remove rows, empty-state, opens `CsvPresetDialog`; compute the taken-identifiers set (excluding the row being edited) for uniqueness; persist the whole array via `settings.set('csv.presets', …)` and read via `settings.subscribe<CsvPreset[]>('csv.presets')`. Depends on T005.
- [X] T007 [US1] Register the category in `src/lib/settings/index.ts`: import `CsvPresetsPage`, `settings.registerSection({id:'csv', label:'settings.section.csv', order:6, component: CsvPresetsPage})` and `settings.register('csv', {key:'csv.presets', type:'custom', default: DEFAULT_CSV_PRESETS, label:'settings.section.csv'})`. Depends on T006.
- [X] T008 [US1] Run `npx svelte-check --tsconfig ./tsconfig.json` and fix all issues; manually verify create/edit/remove/reorder/conditional-inputs/validation/persistence per quickstart "Validate US1".

**Checkpoint**: US1 is fully functional and testable on its own (presets are persisted configuration even before export exists).

---

## Phase 4: User Story 2 - Export to CSV (Priority: P2)

**Goal**: File → Export to CSV opens a destination folder picker and writes one `<identifier>.csv` per preset, with cell values read only from the metadata store; photos without a record are skipped and reported. Scope in this story = the current folder (selection precedence is US3).

**Independent Test**: With presets configured and a folder whose photos have store records, export → confirm one CSV per preset with correct headers, DB-sourced cells, `none`/bool formatting, RFC 4180 escaping, the chosen delimiter, and the no-presets guard (quickstart "Validate US2").

### Implementation for User Story 2

- [X] T009 [P] [US2] Add the pure read `DbState::fetch(&self, path: &str) -> Option<StoredMetadata>` to `src-tauri/src/store/mod.rs` (reuse the private `read_record`; NO writes, NO fingerprint/mtime refresh; `None` when store disabled or row absent). Add a `#[cfg(test)]` test asserting `Some`/`None` and that the DB is unchanged after `fetch`.
- [X] T010 [P] [US2] Define the Rust mirror types in `src-tauri/src/csv/mod.rs`: `AppValueType`, `Delimiter`, `BoolFormat`, `CsvField`, `CsvPreset`, and `ExportSummary` — all `#[serde(rename_all = "camelCase")]`, per data-model.md. `Delimiter` exposes a `byte()` helper (`,`/`;`/`\t`); `BoolFormat` exposes `render(bool) -> &str`.
- [X] T011 [P] [US2] Implement the RFC 4180 writer in `src-tauri/src/csv/writer.rs`: write a row given fields + delimiter byte; quote a field iff it contains the delimiter, `"`, `\r`, or `\n`; escape by wrapping in `"` and doubling embedded `"`; UTF-8 output to a `String`/`Vec<u8>`. Unit tests cover comma/semicolon/tab and each quoting trigger (FR-014, SC-007). Depends on T010 (`Delimiter`).
- [X] T012 [P] [US2] Implement value-type→cell mapping in `src-tauri/src/csv/cell.rs`: `cell(field, path, meta) -> String` per research R10 (`none`→`defaultValue`; `fileName`→basename of `path`; `title`/`description`→as-is; `keywords`→`Vec` joined with `,`; `category`→`categories` normalized (split on commas, trim, drop empties, re-join with `,`); bool types→`boolFormat.render`); empty/missing → empty string (FR-015). Unit tests for every value type incl. both bool formats, the keyword join, and the category normalization. Depends on T010.
- [X] T013 [US2] Implement the `export_csv` and `pick_export_dir` commands in `src-tauri/src/csv/mod.rs` per contracts/ipc-commands.md: `pick_export_dir` uses `app.dialog().file().pick_folder` (mirror `open_folder`); `export_csv` does `let db = db.share();` then a single `spawn_blocking` — fetch each path once (skip+count `None`), and for each preset (every preset has ≥1 field per FR-036) build header+rows via `cell.rs`+`writer.rs` and write `<sanitized identifier>.csv` into `dir` with `std::fs` (overwrite), applying a defensive identifier-sanitize guard (with a `#[cfg(test)]` unit test for the guard: valid/invalid/reserved names); return `ExportSummary`; log IO errors via `log::error!`. Depends on T009, T010, T011, T012.
- [X] T014 [US2] Register `csv::pick_export_dir` and `csv::export_csv` in the `tauri::generate_handler![...]` list in `src-tauri/src/lib.rs`; run `cargo test` (writer/cell/fetch green). Depends on T013.
- [X] T015 [P] [US2] Implement the IPC wrappers in `src/lib/csv/export.ts`: `pickExportDir(): Promise<string | null>` and `exportCsv(dir, paths, presets): Promise<ExportSummary>` (typed `invoke`, mirroring `src/lib/store/metadata.ts`).
- [X] T016 [US2] Add export i18n keys to `src/lib/i18n/types.ts`/`en.ts`/`ru.ts`: `menu.file.exportCsv`, `dialog.exportCsv.title`, a plural result summary key (files written / photos exported / skipped), and warnings `dialog.exportCsv.noPresets` and `dialog.exportCsv.empty` (reuse `common.close`). Shared i18n files — sequence after T003.
- [X] T017 [US2] Wire export into `src/routes/+page.svelte`: add the "Export to CSV" `MenuItem` to the File `MenuTab`; implement `async function exportCsv()` — read `csv.presets` from settings (guard empty → `ConfirmDialog` warning `dialog.exportCsv.noPresets`, FR-029); resolve scope = current-folder photos `panelState.fileTree.children` filtered to non-dir image files in displayed order (selection handled in US3); guard empty scope → warning `dialog.exportCsv.empty` (FR-030); `await pickExportDir()` and abort on `null`; `await exportCsv(dir, paths, presets)`; show the result `ConfirmDialog` (files/exported/skipped, FR-031); log failures via `@tauri-apps/plugin-log`. Depends on T015, T016, T014.
- [X] T018 [US2] Run `cargo test` + `npx svelte-check`; validate US2 + delimiter behavior per quickstart ("Validate US2", "Validate delimiter").

**Checkpoint**: US1 and US2 both work; export of the current folder produces correct CSVs from store data.

---

## Phase 5: User Story 3 - Choose export scope (Priority: P3)

**Goal**: When photos are selected, export only the selection; otherwise the whole current folder (non-recursive).

**Independent Test**: Select 3 of 10 → 3 rows; clear selection with a sub-folder present → only the 10 root photos; a folder where 2 of 10 lack a record → 8 rows + "2 skipped" (quickstart "Validate US3").

### Implementation for User Story 3

- [X] T019 [US3] Extend the scope resolution in `exportCsv()` in `src/routes/+page.svelte`: if `panelState.selectedPaths` is non-empty, scope = those paths in displayed order; else fall back to the current-folder photo list from US2 (non-recursive). Keep the empty-scope guard. Depends on T017.
- [X] T020 [US3] Validate US3 per quickstart ("Validate US3"): selection vs. folder, non-recursive sub-folder exclusion, skipped-count reporting; `npx svelte-check` clean.

**Checkpoint**: All three stories independently functional.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, optional enhancement, and full validation.

- [X] T021 [P] Add a CSV-export section to `static/Help.en.md` and mirror it into `static/Help.ru.md` (EN source of truth per the constitution): menu trigger, preset configuration, value types, per-preset delimiter, scope rules, and the skip-missing-records behavior.
- [ ] T022 [P] (Optional) Add drag-and-drop reordering of fields to `src/lib/settings/CsvPresetDialog.svelte` by lifting the pointer-DnD ghost+placeholder pattern from `src/lib/panel/MetadataPanel.svelte` (keyword chips). Arrows already satisfy FR-026; this is an enhancement, no new dependency.
- [X] T023 Final validation: `cargo test` (writer/cell/fetch/sanitize) and `npx svelte-check` both green; run quickstart.md end-to-end across US1–US3.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies.
- **Foundational (Phase 2)**: depends on Setup; blocks US1 and US2.
- **US1 (Phase 3)**: depends on Foundational. Independently completable (MVP).
- **US2 (Phase 4)**: depends on Foundational. Independent of US1 at runtime, but US2 has no user value without at least one preset (create one in US1 or via the dialog) — it is still independently testable with a manually-created preset.
- **US3 (Phase 5)**: refines US2's `exportCsv()` scope branch — depends on T017 (US2). Behavior is independently testable.
- **Polish (Phase 6)**: depends on the desired stories being complete.

### Within Each Story

- US1: T003/T004 [P] → T005 → T006 → T007 → T008.
- US2: (T009 [P], T010 [P]) → (T011 [P], T012 [P]) → T013 → T014; T015 [P] and T016 anytime after Foundational; T017 after T014/T015/T016; T018 last.
- US3: T019 → T020.

### Parallel Opportunities

- US1: T003 (i18n) ∥ T004 (csv.ts helpers) — different files.
- US2: T009 (store/mod.rs) ∥ T010 (csv/mod.rs types); then T011 (writer.rs) ∥ T012 (cell.rs); T015 (export.ts) ∥ the Rust work.
- Polish: T021 (docs) ∥ T022 (optional DnD).
- **i18n caution**: T003 and T016 edit the same files (`types.ts`/`en.ts`/`ru.ts`) — do them sequentially, never concurrently.

---

## Parallel Example: User Story 2 (Rust core)

```bash
# After Foundational, launch the independent Rust pieces together:
Task: "T009 DbState::fetch pure read in src-tauri/src/store/mod.rs"
Task: "T010 Rust mirror types in src-tauri/src/csv/mod.rs"
# Then, once T010 lands:
Task: "T011 RFC 4180 writer in src-tauri/src/csv/writer.rs"
Task: "T012 value-type->cell mapping in src-tauri/src/csv/cell.rs"
```

---

## Implementation Strategy

### MVP First

1. Phase 1 Setup → Phase 2 Foundational.
2. Phase 3 US1 (presets configuration) — **STOP and VALIDATE** the Settings category independently. This is the P1 MVP slice.
3. Phase 4 US2 — first end-to-end user value: export the current folder to CSV. Validate.
4. Phase 5 US3 — selection-aware scope. Validate.

### Incremental Delivery

Foundational → US1 (configurable presets) → US2 (export current folder) → US3 (scoped export) → Polish. Each step is independently testable and adds value without breaking the previous.

---

## Notes

- [P] = different files, no incomplete dependencies.
- Rust unit tests are inline `#[cfg(test)]` (T009/T011/T012 carry their own tests) per repo convention — no separate TDD phase.
- Run `npx svelte-check` after every frontend change (constitution Development Workflow); commit per Spec Kit phase (Principle VII).
- No new dependency is introduced; the CSV writer is hand-rolled (research R1).
- Export reads strictly from the store (`DbState::fetch`); never re-read photo files (FR-009/FR-028).
