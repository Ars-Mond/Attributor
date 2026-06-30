# Phase 0 Research: CSV Export

All unknowns from the Technical Context resolved below. Codebase facts were gathered by reading the existing Settings/Ollama, store, menu, dialog, and selection code.

## R1. CSV writing — hand-rolled writer vs. the `csv` crate

**Decision**: Hand-roll a small, pure-Rust RFC 4180 writer in `src-tauri/src/csv/writer.rs`. No dependency.

**Rationale**:
- The full surface needed is tiny: write a header row and N data rows, with a configurable single-byte delimiter (comma/semicolon/tab) and RFC 4180 quoting. A field is quoted iff it contains the delimiter, a `"`, a `\r`, or a `\n`; quoting wraps the field in `"` and doubles any embedded `"`. That is ~30 lines and is fully unit-testable (FR-014, SC-007).
- Principle X ("deliberate, small dependency set") favors not adding a crate for something this small. The store already justified `rusqlite`/`xxhash`; a CSV crate is not similarly load-bearing.
- Keeping it in-tree lets us match exactly the per-preset delimiter + global UTF-8 + always-quote-when-needed behavior with no surprises.

**Alternatives considered**:
- **`csv` crate (BurntSushi)** — pure Rust, robust, `WriterBuilder::delimiter(b';')` + automatic quoting. Rejected only to avoid a new dependency; it would also satisfy Principle I. Reconsider if requirements grow (e.g., CSV *reading*, streaming, or exotic quoting modes).
- **Manual `format!`/`join` without quoting** — rejected: fails FR-014/SC-007 for values containing delimiters, quotes, or newlines.

**Encoding**: UTF-8 without BOM, written via `std::fs`. (Excel-on-Windows BOM concerns are out of scope; can be revisited if a stock demands it.)

## R2. Pure read from the metadata store (no mutation)

**Decision**: Add `DbState::fetch(&self, path: &str) -> Option<StoredMetadata>` to `store/mod.rs`, reusing the already-private, already-pure `read_record` (a single `SELECT ... WHERE path = ?1` via `.optional()`), mapping its internal `Record.meta` to `StoredMetadata`. Returns `None` when the store is disabled or the row is absent.

**Rationale**: Export must read strictly from the store and must NOT write or refresh fingerprints/mtime (FR-009/FR-028). `read_record` already does exactly this; `resolve_open` does NOT (it upserts/refreshes), so it must not be used. A thin public accessor avoids duplicating SQL and avoids touching existing paths.

**Alternatives considered**:
- Reuse `open_metadata`/`resolve_open` — rejected: mutates the DB (creates rows, refreshes mtime).
- A bulk `SELECT ... WHERE path IN (...)` — deferred: the single `Arc<Mutex<Connection>>` serializes anyway, and a sequential per-path `fetch` inside one `spawn_blocking` is simple and fast enough for the scale (R6). Bulk can be a later optimization if profiling demands.

## R3. Photos without a store record

**Decision**: Skip them — do not write a row, do not read the file — and count them; return the count in the export summary (FR-035, clarified 2026-06-30).

**Rationale**: Keeps export strictly DB-sourced and predictable. The user reviews/saves photos (which creates store records) before exporting; the summary tells them how many were skipped so nothing is silently lost.

## R4. Destination selection — folder picker

**Decision**: Add a Rust command `pick_export_dir() -> Result<Option<String>, String>` that calls `app.dialog().file().pick_folder(...)` (the same `tauri-plugin-dialog` pattern as the existing `open_folder` in `lib.rs`), returning the chosen absolute path or `None` on cancel. The frontend invokes it, aborts on `None`, then invokes `export_csv`.

**Rationale**: The frontend never imports `@tauri-apps/plugin-dialog` in this project; all dialogs and all file writes go through Rust commands (there is no `fs` plugin permission). A dedicated folder picker mirrors `open_folder` without its scan side effects and keeps the picker decoupled from the write step (more testable).

**Alternatives considered**: Folding the picker into `export_csv` — rejected: mixes UI dialog with heavy work, harder to test; a cancel would be awkward to express in the summary type.

## R5. Where presets live and how Rust gets them

**Decision**: Presets persist via `tauri-plugin-store` under `settings.json` key `csv.presets` (mirroring `ollama.modelProfiles`). For export, the **frontend reads the presets and passes them as a typed argument** to `export_csv`; Rust stays stateless about preset storage.

**Rationale**: Matches the established settings architecture (custom-component section + a registered custom key). Passing presets as a serde-typed argument keeps the Rust command pure and avoids coupling Rust to the store-file schema, while remaining typed per Principle IX.

**Alternatives considered**: Reading `settings.json` from Rust via the plugin's Rust API — rejected: couples backend to the FE store schema and key names; no benefit at this scale.

## R6. Concurrency / performance pattern

**Decision**: In `export_csv`, resolve scope on the frontend (it owns selection + folder), pass the ordered `paths: Vec<String>` + `presets` to Rust. In Rust: `let db = db.share();` then a single `tokio::task::spawn_blocking` that (a) sequentially `fetch`es each path's record once, (b) for each preset formats rows and writes `<identifier>.csv` with `std::fs`. One IPC round-trip total.

**Rationale**: Principle VIII — heavy work in Rust, no IPC in loops, batch once. The SQLite connection is a single `Arc<Mutex<Connection>>`, so DB reads serialize regardless; a sequential loop is the cleanest correct option (the poison-tolerant `lock().unwrap_or_else(|e| e.into_inner())` idiom is used project-wide). Fetch each path once, reuse the record across all presets. `rayon` is unnecessary here (DB-bound); reserve it only if pure string formatting ever dominates.

**Performance**: A few thousand single-row SELECTs plus buffered file writes complete well under the 5 s / 1,000-photos × 3-presets target (SC-008); the blocking task keeps the UI thread free. No dedicated performance-measurement task is added — SC-008 is satisfied at the design level (analyze decision, 2026-07-01).

## R7. Export scope resolution (selection vs. current folder)

**Decision**: Resolve on the frontend before invoking. If `panelState.selectedPaths` is non-empty, scope = those paths; else scope = `panelState.fileTree.children` filtered to non-dir image files (`isImageFile`), in stored order. Pass the resulting ordered `string[]` to `export_csv`.

**Rationale**: The frontend already holds both the multi-select set and the current-folder tree; the backend scan defines the order (directories first, then files by name) which is exactly what the user sees (FR-005). Resolving in the FE keeps `export_csv` a pure "given these paths, write CSVs" function.

**Edge guards (frontend, before invoking)**: no presets configured → `ConfirmDialog` warning, no call (FR-029); empty scope → `ConfirmDialog` warning, no call (FR-030).

## R8. Field reordering UI

**Decision**: Implement up/down **arrow** controls on each field row (array `splice` swap) as the baseline, satisfying FR-026 ("arrows and/or drag-and-drop"). Drag-and-drop is an optional enhancement that can lift the existing hand-rolled pointer-DnD pattern from `MetadataPanel.svelte` (keyword chips) — no library, no dependency.

**Rationale**: Arrows are trivial, accessible, and fully satisfy the requirement; DnD is nice-to-have and reuses an in-repo pattern if added. No new dependency either way.

## R9. Identifier validation (uniqueness + file-name safety)

**Decision**: Validate in `CsvPresetDialog` using the established inline-error pattern (uniqueness candidate set computed by the parent page, excluding the row being edited). Reject empty identifiers, duplicates (FR-019), and identifiers containing any character illegal in a file name on any supported OS — the conservative reject set `< > : " / \ | ? *`, control chars, leading/trailing dots or spaces, and reserved Windows device names (CON, PRN, AUX, NUL, COM1–9, LPT1–9) (FR-020). The same sanitization rule is mirrored in Rust as a defensive guard before writing `<identifier>.csv`.

**Rationale**: Cross-platform parity (Principle IV) requires the strictest common subset so a preset created on Linux still produces a valid file name on Windows. Front-loading validation in the dialog gives immediate feedback; the Rust guard prevents a malformed identifier from ever reaching the filesystem.

## R10. Value-type -> cell mapping (from `StoredMetadata`)

**Decision**: Map each `AppValueType` to a cell string in `csv/cell.rs`:

| Value type | Source | Cell |
|------------|--------|------|
| none | (constant) | the field's `defaultValue` string |
| fileName | photo path | file name with extension (basename) |
| title | `StoredMetadata.title` | as-is |
| description | `StoredMetadata.description` | as-is |
| keywords | `StoredMetadata.keywords: Vec<String>` | joined with `,` (in-cell comma, quoted by the writer when needed) |
| category | `StoredMetadata.categories: String` | normalized: split on commas, trim each, drop empties, re-join with `,` |
| releaseFilename | `StoredMetadata.release_filename: String` | as-is |
| editorial | `StoredMetadata.editorial: bool` | per field `boolFormat`: `yes`/`no` or `true`/`false` |
| matureContent | `StoredMetadata.mature_content: bool` | per field `boolFormat` |
| illustration | `StoredMetadata.illustration: bool` | per field `boolFormat` |

Missing/empty values → empty cell (FR-015). The in-cell keyword separator is always `,` regardless of the preset's column delimiter (clarified 2026-06-30). The stored `categories` string is normalized to a comma separator on export (split on commas, trim each, drop empties, re-join with `,`) so multi-value category cells match the keyword convention (FR-012).

**Rationale**: Mirrors the exact store field set and the clarified serialization rules; the file name comes from the path (the only non-store value), satisfying "from the database" for all metadata while still emitting the asset file name.
