# Phase 1 Contracts: IPC Commands — CSV Export

Two new Tauri commands, registered in `src-tauri/src/lib.rs` `invoke_handler`. Both follow project conventions: `#[tauri::command] pub async fn`, return `Result<T, String>`, managed state last, camelCase serde on all crossing types (Principle IX). Frontend wrappers live in `src/lib/csv/export.ts` and use the typed `invoke` pattern from `src/lib/store/metadata.ts`.

---

## Command: `pick_export_dir`

Open a native folder picker and return the chosen destination directory.

**Rust signature**:
```rust
#[tauri::command]
pub async fn pick_export_dir(app: tauri::AppHandle) -> Result<Option<String>, String>
```

**Behavior**:
- Calls `app.dialog().file().pick_folder(...)` (same pattern as `open_folder` in `lib.rs`).
- Resolves to `Some(absolute_path)` on selection, `None` on cancel.
- No filesystem side effects.

**Frontend wrapper**:
```ts
export async function pickExportDir(): Promise<string | null> {
  return await invoke<string | null>('pick_export_dir');
}
```

---

## Command: `export_csv`

Write one CSV file per preset into `dir`, using store records for the given `paths`.

**Request** (camelCase args):
```ts
interface ExportCsvArgs {
  dir: string;            // destination folder (from pick_export_dir)
  paths: string[];        // ordered, resolved scope (selection or current folder), as displayed
  presets: CsvPreset[];   // the full csv.presets array (see data-model.md)
}
```

**Rust signature**:
```rust
#[tauri::command]
pub async fn export_csv(
    dir: String,
    paths: Vec<String>,
    presets: Vec<CsvPreset>,
    db: tauri::State<'_, crate::store::DbState>,
) -> Result<ExportSummary, String>
```

**Response**:
```ts
interface ExportSummary {
  filesWritten: number;
  photosExported: number;
  skipped: number;
}
```

**Behavior** (one IPC round-trip, all work in `spawn_blocking` after `db.share()`):
1. For each `path` in `paths` (in order), `db.fetch(path)` once → `Some(record)` or `None`. Paths with `None` are counted as `skipped` and excluded from every file (FR-035).
2. For each `preset` with ≥1 field:
   - Build the header row from `field.csvColumn` in order (FR-007).
   - For each photo with a record, build a data row by mapping each field's `valueType` to a cell (see data-model.md R10): `none`→`defaultValue`; `fileName`→basename of `path`; text fields→store strings; `keywords`→`Vec` joined with `,`; `category`→`categories` as-is; bool fields→`boolFormat` (`yes`/`no` or `true`/`false`). Empty/missing → empty cell (FR-015).
   - Write `<sanitized identifier>.csv` into `dir` via `std::fs`, UTF-8, using the preset's `delimiter` and RFC 4180 quoting (a field is quoted iff it contains the delimiter, `"`, `\r`, or `\n`) (FR-014/FR-033/FR-034). Overwrite if it already exists.
   - Increment `filesWritten`.
3. `photosExported` = number of in-scope photos that had a record. Return `ExportSummary`.

**Errors** (`Err(String)`, surfaced + logged per FR-032):
- `dir` not writable / file write failure.
- Invalid/unsafe `identifier` reaching the backend guard (should be prevented in the dialog; defensive).
- Store disabled (`db` has no connection) → treat all photos as `skipped` (graceful), not a hard error.

**Empty-input expectations** (frontend guards before calling, FR-029/FR-030): the page does not invoke `export_csv` when there are no presets or the scope is empty; it shows a `ConfirmDialog` warning instead. If called anyway, the command returns a zeroed `ExportSummary` without writing files.

**Frontend wrapper**:
```ts
export async function exportCsv(dir: string, paths: string[], presets: CsvPreset[]): Promise<ExportSummary> {
  return await invoke<ExportSummary>('export_csv', {dir, paths, presets});
}
```

---

## Registration

Add to the `tauri::generate_handler![...]` list in `src-tauri/src/lib.rs`:
```rust
csv::pick_export_dir,
csv::export_csv,
```
`DbState` is already managed in `.setup(...)`; no extra `.manage()` is required.
