# Contract: Tauri IPC Commands

Signatures are **unchanged**; only the implementation switches from the `xmp` module to the `photo`
module. Both keep `Result<T, String>`, `serde` DTOs, and `#[serde(rename_all = "camelCase")]`
(Principle IX). No new command is added (image decode stays backend-only — Q4 / FR-009).

## `read_metadata`

```rust
#[tauri::command]
fn read_metadata(path: String) -> Result<ReadResult, String>
```

- **In**: absolute file path.
- **Out**: `ReadResult { title, description, keywords, categories, releaseFilename }`.
- **Behavior**: `Photo::open(path)?.read_metadata()?` → map `photo::Metadata` to `ReadResult`
  (`category → categories`, `releaseFilename = ""`). Streaming read; merged EXIF/IPTC/XMP.
- **Errors**: unsupported format or I/O failure → `Err(String)` (logged). Missing metadata is **not**
  an error — returns empty fields.

## `save_metadata`

```rust
#[tauri::command]
fn save_metadata(metadata: SaveRequest) -> Result<String, String>
```

- **In**: `SaveRequest { filepath, filename, title, description, keywords, categories, releaseFilename }`.
- **Out**: final file path (new path if renamed, original otherwise).
- **Behavior**: map `SaveRequest` → `photo::Metadata` (`categories → category`); `Photo::open(filepath)?`
  then `save_metadata(&meta)` writing EXIF+IPTC+XMP duplicated, removing cleared fields. Preserve the
  current rename flow (atomic `create_new` when the stem changes, then delete original) and
  `releaseFilename` passthrough (not persisted to metadata).
- **Errors**: target name collision (`File already exists`), unsupported format, or I/O failure →
  `Err(String)` (logged). Never panics across the boundary.

## Registration

`tauri::generate_handler![ read_metadata, save_metadata, open_folder, open_folder_path, scan_folder, search_keywords ]`
— unchanged set; `read_metadata` / `save_metadata` now delegate to `photo`.
