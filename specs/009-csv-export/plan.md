# Implementation Plan: CSV Export

**Branch**: `009-csv-export` | **Date**: 2026-06-30 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/009-csv-export/spec.md`

## Summary

Add a "Export to CSV" capability. The user configures one or more **stock CSV presets** in a new Settings category (mirroring the existing Ollama-models category): each preset has a display name, a unique file-name-safe stock identifier, a column delimiter (comma/semicolon/tab, default comma), and an ordered, add/remove/reorder list of fields. Each field maps a CSV column header to an app value type (none, file name, title, description, keywords, category, editorial, mature content, illustration); `none` fields carry a constant default value, bool fields carry a yes/no-vs-true/false format choice.

Triggered from the File menu, export opens a destination **folder** picker, resolves the scope (selected photos, else every photo in the current folder excluding sub-folders), and writes one `<identifier>.csv` per preset into that folder. Cell values are read **only from the SQLite metadata store** (feature 008) — photos without a store record are skipped and reported. All row building, RFC 4180 quoting, and file writing happen in Rust behind a single IPC call.

## Technical Context

**Language/Version**: Rust (edition 2021) backend; TypeScript 5.6 + Svelte 5 (runes) frontend.

**Primary Dependencies**: Tauri 2, `tauri-plugin-store` (preset persistence in `settings.json`), `tauri-plugin-dialog` (folder picker, via the existing Rust `pick_folder` pattern), `rusqlite` (existing 008 store — read only), `tauri-plugin-log`. **No new crate** — a small hand-rolled RFC 4180 CSV writer is used (see research.md).

**Storage**:
- Presets → `tauri-plugin-store` (`settings.json`, key `csv.presets`), exactly like `ollama.modelProfiles`.
- Export source data → the SQLite metadata store (`metadata.db`, table `photo_metadata`), read-only.
- Output → `<identifier>.csv` files written to the user-chosen folder via `std::fs`.

**Testing**: `cargo test` (Rust unit tests for the CSV writer, value-type→cell mapping, identifier sanitization, and the pure `DbState::fetch` read); `npx svelte-check` for the frontend.

**Target Platform**: Windows, Linux, macOS desktop (Tauri 2 packaged app).

**Project Type**: desktop-app (Tauri 2 + SvelteKit/Svelte 5).

**Performance Goals**: Export 1,000 photos × 3 presets in under 5 s while keeping the UI responsive (SC-008): one IPC round-trip, all work in a `spawn_blocking` task.

**Constraints**: Values strictly from the store, never re-read from files (FR-009/FR-028); no IPC inside hot loops (Principle VIII); cross-platform file-name safety for identifiers (Principle IV); per-preset delimiter with global UTF-8 + RFC 4180 quoting (Clarifications).

**Scale/Scope**: Typical folders of a few hundred to a few thousand photos; a handful of presets, each with a handful of fields.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Pure Rust Backend | PASS | No new dependency; CSV writer is hand-rolled pure Rust. Store reads reuse `rusqlite` (already justified in 008). |
| II. Modern Svelte 5 (Runes) | PASS | New `CsvPresetsPage`/`CsvPresetDialog` use `$state`/`$derived`/`$props`; no legacy stores or `export let`. |
| III. Themed SCSS Tokens | PASS | Reuse `$fs-*`, `$radius-*`, color tokens, and the `.mp-row`/`.md-input`/`.mp-btn` recipes; no hardcoded values. |
| IV. Cross-Platform Parity | PASS | Identifier sanitization rejects the union of illegal characters across OSes; folder picker + `std::fs` behave identically. |
| V. Reuse UI Primitives | PASS | Reuse `MenuItem`, `ConfirmDialog`, the settings registry, and the existing folder-pick Rust pattern. |
| VI. Mandatory Logging | PASS | Rust logs write/IO failures via `log`; frontend uses `@tauri-apps/plugin-log` (`warn`/`error`), never `console.*`. |
| VII. Phase-Based Commits | PASS | One commit per Spec Kit phase via `/speckit-git-commit`. |
| VIII. Rust Performance First | PASS | Row building, quoting, and file writes in Rust; a single `export_csv` IPC call; `db.share()` + `spawn_blocking`. |
| IX. Typed Tauri IPC | PASS | `pick_export_dir` / `export_csv` return `Result<T, String>`, with `#[serde(rename_all = "camelCase")]` types. |
| X. Fixed Stack | PASS | No new dependency added; if the `csv` crate were ever adopted it would need justification — avoided here. |
| XI. Code Style | PASS | English comments/identifiers; no inner brace spaces in TS; no alignment padding. |

**Communication & Documentation**: `static/Help.en.md` and `static/Help.ru.md` get a CSV-export section (EN source of truth mirrored to RU) in the polish phase. Spec Kit artifacts are English.

**Result**: No violations. Complexity Tracking is empty.

## Project Structure

### Documentation (this feature)

```text
specs/009-csv-export/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   └── ipc-commands.md  # Phase 1 output — pick_export_dir, export_csv
├── checklists/
│   └── requirements.md  # /speckit-specify output
└── tasks.md             # /speckit-tasks output (not created here)
```

### Source Code (repository root)

```text
src-tauri/src/
├── csv/                      # NEW module — CSV export
│   ├── mod.rs                # export_csv + pick_export_dir commands, ExportSummary, preset structs
│   ├── writer.rs             # hand-rolled RFC 4180 CSV writer (configurable delimiter) + unit tests
│   └── cell.rs               # AppValueType -> cell-string mapping (+ bool format, keyword join) + tests
├── store/
│   └── mod.rs                # ADD `DbState::fetch(path) -> Option<StoredMetadata>` (pure read; reuses read_record)
└── lib.rs                    # register `csv::export_csv`, `csv::pick_export_dir` in invoke_handler

src/lib/
├── csv/
│   ├── csv.ts                # CsvPreset / CsvField / AppValueType / Delimiter / BoolFormat types + DEFAULT_CSV_PRESETS + sanitize/validate helpers
│   └── export.ts             # invoke wrappers: pickExportDir(), exportCsv(dir, paths, presets)
├── settings/
│   ├── CsvPresetsPage.svelte # NEW — mirrors OllamaModelsPage (list add/edit/remove)
│   ├── CsvPresetDialog.svelte# NEW — mirrors OllamaModelDialog (name/identifier/delimiter + field list editor + reorder + validation)
│   └── index.ts              # register `csv` section + `csv.presets` custom key
├── i18n/
│   ├── types.ts              # add CSV message keys to Messages
│   ├── en.ts                 # add EN values
│   └── ru.ts                 # add RU values
└── routes/+page.svelte       # add "Export to CSV" MenuItem + exportCsv() orchestration (scope resolution, guards, result dialog)

static/
├── Help.en.md                # add CSV-export section (polish)
└── Help.ru.md                # mirror RU (polish)
```

**Structure Decision**: Desktop-app layout (Tauri 2 + SvelteKit). A new `src-tauri/src/csv/` module owns the export command, the pure-Rust CSV writer, and the value-type->cell mapping, keeping it isolated and unit-testable. The store gains a single pure-read accessor (`fetch`) — no change to existing read/write paths. The frontend adds a `csv` settings category that mirrors the Ollama-models category file-for-file, a small `src/lib/csv/` glue layer, and a thin export orchestration in the page that already owns the menu and selection state. No new dependencies.

## Complexity Tracking

> No Constitution violations — section intentionally empty.
