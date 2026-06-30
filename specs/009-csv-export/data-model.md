# Phase 1 Data Model: CSV Export

This feature adds **configuration** entities (persisted in `settings.json` via `tauri-plugin-store`) and a transient **export result**. It reads, but does not change, the existing `StoredMetadata` from feature 008. Field names are camelCase across the IPC/store boundary (Principle IX); Rust structs use `#[serde(rename_all = "camelCase")]`.

## Entity: CsvPreset

One photo-stock's export configuration. Persisted as an element of the `csv.presets` array in `settings.json`. Analogue of `ModelProfile`.

| Field | Type | Notes |
|-------|------|-------|
| `id` | string | Stable internal id (generated, e.g. `crypto.randomUUID()`); identity for edit/remove and reorder. Never shown. |
| `name` | string | Display name shown in Settings (FR-018). May be empty-checked as required. |
| `identifier` | string | Stock identifier — used as the CSV file name `<identifier>.csv` and as the uniqueness key (FR-018/FR-019/FR-020). |
| `delimiter` | `Delimiter` | Column delimiter for this preset's file (FR-034). Default `comma`. |
| `fields` | `CsvField[]` | Ordered list of columns (FR-021). Order = column order in the output (FR-007). MUST contain at least one field — saving an empty list is rejected in the dialog (FR-036). |

**Validation rules**:
- `name` required (non-empty after trim).
- `identifier` required; unique across all presets (case-insensitive compare recommended) (FR-019); file-name-safe per R9 (FR-020).
- `fields` MUST be non-empty; the dialog blocks saving a preset with zero fields (FR-036).
- Reordering changes only the order of `fields`; add/remove changes membership (FR-021/FR-026).

**Persistence shape** (example element of `csv.presets`):
```json
{
  "id": "b1f2…",
  "name": "Shutterstock",
  "identifier": "shutterstock",
  "delimiter": "comma",
  "fields": [
    {"csvColumn": "Filename", "valueType": "fileName", "defaultValue": "", "boolFormat": "yesNo"},
    {"csvColumn": "Description", "valueType": "description", "defaultValue": "", "boolFormat": "yesNo"},
    {"csvColumn": "Keywords", "valueType": "keywords", "defaultValue": "", "boolFormat": "yesNo"},
    {"csvColumn": "Categories", "valueType": "category", "defaultValue": "", "boolFormat": "yesNo"},
    {"csvColumn": "Editorial", "valueType": "editorial", "defaultValue": "", "boolFormat": "yesNo"},
    {"csvColumn": "Mat", "valueType": "none", "defaultValue": "no", "boolFormat": "yesNo"}
  ]
}
```

## Entity: CsvField

One column within a preset.

| Field | Type | Notes |
|-------|------|-------|
| `csvColumn` | string | The CSV header text for this column (FR-007/FR-022). |
| `valueType` | `AppValueType` | What fills the cell (FR-022/FR-023). |
| `defaultValue` | string | Constant emitted for every row — **used only when `valueType == none`** (FR-010/FR-024); ignored otherwise but always stored (avoids conditional shape). |
| `boolFormat` | `BoolFormat` | How a bool renders — **used only for bool value types** (FR-011/FR-025); ignored otherwise but always stored. Default `yesNo`. |

**UI conditional display** (FR-024/FR-025): the `defaultValue` input is shown only for `none`; the `boolFormat` control is shown only for `editorial`/`matureContent`/`illustration`. The fields still persist with defaults regardless, so switching a field's type never loses prior input unexpectedly and the stored shape is uniform.

## Enum: AppValueType

Fixed set (FR-023). Serialized as camelCase strings.

`none` | `fileName` | `title` | `description` | `keywords` | `category` | `editorial` | `matureContent` | `illustration`

Bool types = `editorial`, `matureContent`, `illustration`.

Rust:
```rust
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum AppValueType {
    None, FileName, Title, Description, Keywords, Category,
    Editorial, MatureContent, Illustration,
}
```

## Enum: Delimiter

`comma` | `semicolon` | `tab` (FR-034). Maps to a single byte: `,` / `;` / `\t`. Default `comma`.

## Enum: BoolFormat

`yesNo` | `trueFalse` (FR-011). `yesNo` → `yes`/`no`; `trueFalse` → `true`/`false`. Default `yesNo`.

## Entity: ExportSummary (transient, command result)

Returned by `export_csv`; drives the result `ConfirmDialog` (FR-031).

| Field | Type | Notes |
|-------|------|-------|
| `filesWritten` | number | Count of CSV files written (one per preset; every preset has ≥1 field per FR-036). |
| `photosExported` | number | In-scope photos that had a store record and produced a data row. |
| `skipped` | number | In-scope photos with no store record (FR-035). |

Rust:
```rust
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExportSummary {
    pub files_written: usize,
    pub photos_exported: usize,
    pub skipped: usize,
}
```

## Referenced (unchanged): StoredMetadata (feature 008)

Read-only source for cell values. Frontend type `src/lib/types.ts`; Rust `store/record.rs`.

| Field | Type | Used by value type |
|-------|------|--------------------|
| `title` | string | title |
| `description` | string | description |
| `keywords` | string[] (`Vec<String>`) | keywords (joined with `,`) |
| `categories` | string | category — normalized: split on commas, trim each, drop empties, re-join with `,` |
| `editorial` | bool | editorial |
| `matureContent` | bool | matureContent |
| `illustration` | bool | illustration |
| `releaseFilename` | string | (not exposed as a value type in this version) |

The photo's **file name** (for `fileName`) comes from the photo path, not from `StoredMetadata`.

## Defaults

`DEFAULT_CSV_PRESETS` (seed for the registered `csv.presets` key) = `[]` (empty). The user creates presets explicitly; export with no presets shows the "nothing to export" warning (FR-029). A non-destructive starter template MAY be offered by the "Create" action but is not seeded by default.

## Relationships & lifecycle

- A `CsvPreset` **has many** `CsvField` (ordered, owned).
- An export run reads the current `csv.presets` array and the resolved scope `paths`; it produces one file per preset and one `ExportSummary`. No entity is mutated by export.
- Presets are created/edited/deleted/reordered only through the Settings CSV category; each change rewrites the whole `csv.presets` array via `settings.set` (same pattern as `ollama.modelProfiles`).
