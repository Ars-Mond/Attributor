# Feature Specification: CSV Export

**Feature Branch**: `009-csv-export`

**Created**: 2026-06-30

**Status**: Draft

**Input**: User description: "Add CSV export. A menu item triggers a destination-folder picker; the app exports the selected photos, or — when nothing is selected — every photo in the current folder excluding sub-folders. The folder (not a single file) is chosen because one CSV is written per configured photo-stock. Settings gains a CSV category (like the Ollama models category) where each photo-stock is configured: a display name, a unique stock identifier (used as the file name), and an ordered, editable list of fields. Each field carries a CSV column identifier and an app value type (none, file name, title, description, keywords, category, editorial, mature content, illustration); a default string value exists only for `none` fields, and a bool-format choice (yes/no vs true/false) exists only for the bool types. CSV data is read from the database, not from the files."

## Clarifications

### Session 2026-06-30

- Q: How should multi-value fields (keywords, category) be written into a single CSV cell? → A: Joined with a comma inside a quoted cell — the in-cell separator stays a comma regardless of the preset's column delimiter.
- Q: What should happen to in-scope photos that have no database record? → A: Exclude them from every CSV and report the skipped count in the export outcome; the export stays purely database-sourced.
- Q: What CSV column delimiter is used and where is it configured? → A: UTF-8 and RFC 4180 quoting are global; each preset additionally selects its own column delimiter — comma, semicolon, or tab — defaulting to comma.
- Q: Which presets are exported when the user runs an export? → A: Every configured preset (one CSV per preset).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Configure stock CSV presets (Priority: P1)

A user opens Settings, finds a dedicated CSV category, and creates a preset for a photo-stock. They give the preset a display name and a unique stock identifier, then build an ordered list of columns: for each column they enter the CSV header name and pick what app value fills it. For placeholder columns they type a constant default value; for the yes/no-style flag columns they choose how the boolean is rendered. They reorder columns until the layout matches what that stock expects, and the preset is remembered for next time.

**Why this priority**: Every stock agency demands a different column layout. Without the ability to describe that layout, there is no target format to export into — this is the foundation the rest of the feature builds on, and it delivers standalone value as a persisted, reusable configuration.

**Independent Test**: Open Settings → CSV, create a preset with a name, an identifier, and several fields of different types, reorder them, save, restart the app, and confirm the preset and its exact field order are still present.

**Acceptance Scenarios**:

1. **Given** the CSV settings category is open, **When** the user creates a preset with a display name and stock identifier and adds three fields, **Then** the preset is listed and persists across an app restart.
2. **Given** a preset is being edited, **When** the user adds a field and sets its value type to "none", **Then** a default-value text input appears for that field and a bool-format option does not.
3. **Given** a preset is being edited, **When** the user sets a field's value type to "editorial" (a bool type), **Then** a bool-format option (yes/no vs true/false) appears for that field and a default-value input does not.
4. **Given** a preset has several fields, **When** the user moves a field up/down (arrows) or drags it to a new position, **Then** the field order updates and is preserved on save.
5. **Given** a preset already uses stock identifier "shutterstock", **When** the user tries to create or rename another preset to the same identifier, **Then** the app rejects it and explains the identifier must be unique.
6. **Given** a preset has no fields, **When** the user tries to save it, **Then** the dialog rejects the save and explains at least one field is required.

---

### User Story 2 - Export photos to CSV (Priority: P2)

A user who has configured one or more stock presets selects "Export to CSV" from the menu. A picker asks for a destination **folder** (not a single file). The app writes one CSV file per configured preset into that folder, named after each preset's stock identifier. Each file starts with a header row built from the preset's column identifiers, followed by one row per photo in scope, with each cell filled from that photo's metadata in the database.

**Why this priority**: This is the headline value — turning the app's stored metadata into submission-ready spreadsheets for each agency. It depends on at least one preset existing (US1) but is the reason the feature exists.

**Independent Test**: With at least one preset configured and a folder of photos that have saved metadata, run Export to CSV, choose a destination folder, and verify a `<identifier>.csv` file appears for each preset containing a correct header row plus one row per photo with values matching the database.

**Acceptance Scenarios**:

1. **Given** two presets are configured and a folder with N photos is open, **When** the user exports and picks a destination folder, **Then** two CSV files (named by each preset's identifier) are written there, each with one header row and N data rows.
2. **Given** a preset has a column of type "title", **When** a photo is exported, **Then** that cell contains exactly the photo's title as stored in the database.
3. **Given** a preset has a "none" column with default value "RF", **When** any photo is exported, **Then** that cell contains "RF" for every row.
4. **Given** a preset has an "editorial" column set to yes/no format, **When** a photo whose editorial flag is true is exported, **Then** that cell contains "yes" (and "no" when false).
5. **Given** a cell value contains the delimiter, a quote, or a line break, **When** the file is written, **Then** the value is escaped so the CSV remains well-formed.
6. **Given** no presets are configured, **When** the user selects Export to CSV, **Then** the app informs the user there is nothing to export and writes no files.
7. **Given** a preset's column delimiter is set to semicolon, **When** that preset is exported, **Then** its CSV uses `;` between columns while UTF-8 encoding and quoting stay consistent.
8. **Given** a folder of 10 photos where 2 have no database record, **When** the user exports, **Then** each CSV contains 8 data rows and the outcome message reports 2 skipped photos.

---

### User Story 3 - Choose export scope (Priority: P3)

When the user has selected one or more photos, exporting covers only that selection. When nothing is selected, exporting covers every photo in the current folder, excluding any sub-folders. This lets the user target a precise subset or fall back to the whole folder without extra steps.

**Why this priority**: A usability refinement on top of export. Export is valuable even if it only ever handled the whole folder; selection-awareness sharpens it.

**Independent Test**: With several photos selected, export and confirm only the selected photos produce rows; then clear the selection, export again, and confirm every photo in the current folder (and none from sub-folders) produces rows.

**Acceptance Scenarios**:

1. **Given** 3 of 10 photos in the current folder are selected, **When** the user exports, **Then** each CSV contains exactly 3 data rows for the selected photos.
2. **Given** no photo is selected and the current folder has 10 photos plus a sub-folder with more photos, **When** the user exports, **Then** each CSV contains exactly 10 data rows and none from the sub-folder.

---

### Edge Cases

- **No presets configured**: Export reports that nothing can be exported and writes no files.
- **Empty scope**: The selection is empty and the current folder has no photos → export reports there is nothing to export.
- **Photo without a database record**: A photo in scope has no saved metadata in the database → it is excluded from every CSV and counted in the export outcome (it is not exported with blank values nor read from the file).
- **Missing/empty field value**: A photo has no value for a column's type (e.g., no keywords) → the cell is empty.
- **Duplicate stock identifier**: Creating/renaming a preset to an identifier already in use is rejected at configuration time.
- **Invalid stock identifier**: An identifier containing characters illegal in a file name is rejected with an explanation.
- **Destination already contains same-named files**: Existing `<identifier>.csv` files are overwritten (see Assumptions).
- **Special characters in cell values**: Delimiters, quotes, and newlines inside a value are escaped so the file stays valid.
- **Multi-value fields (keywords, category)**: Joined into a single cell with a comma; the cell is quoted when the value would otherwise break the CSV.
- **Preset with no fields**: A preset must have at least one field; the Settings dialog rejects saving a preset with an empty field list (FR-036), so export never encounters an empty preset.

## Requirements *(mandatory)*

### Functional Requirements

#### Trigger & scope

- **FR-001**: The application MUST expose an "Export to CSV" action in the application menu.
- **FR-002**: Activating the action MUST open a destination **folder** picker (not a single-file save dialog).
- **FR-003**: When one or more photos are selected, the export scope MUST be exactly those selected photos.
- **FR-004**: When no photo is selected, the export scope MUST be every photo in the current folder, excluding photos in sub-folders.
- **FR-005**: Rows in each CSV MUST follow the same order in which the photos appear in the application's folder view.

#### Output files

- **FR-006**: For each configured stock preset, the export MUST write exactly one CSV file into the chosen folder, named `<stock_identifier>.csv`.
- **FR-007**: Each CSV's first row MUST be a header containing the preset's column identifiers, in the preset's configured field order.
- **FR-008**: Each CSV MUST contain one data row per photo in scope.
- **FR-009**: Each cell MUST be derived from its column's app value type, read from the metadata database — never re-read from the photo file.
- **FR-010**: A column of type "none" MUST emit the column's configured default string value in every data row.
- **FR-011**: A column of a bool type (editorial, mature content, illustration) MUST be rendered per the column's bool-format choice — "yes"/"no" or "true"/"false".
- **FR-012**: A column whose app value is multi-valued (keywords, category) MUST be serialized into a single cell by joining the values with a comma; the cell MUST be quoted when needed so the in-cell comma is not read as a column delimiter.
- **FR-013**: A column of type "file name" MUST emit the photo's file name.
- **FR-014**: Cell values containing the delimiter, quote character, or a line break MUST be escaped so the resulting file is a well-formed CSV.
- **FR-015**: Missing or empty values MUST produce an empty cell.
- **FR-033**: Each preset's CSV MUST be written using that preset's configured column delimiter; UTF-8 encoding and RFC 4180 quoting apply to every file regardless of the chosen delimiter.

#### Settings: preset management

- **FR-016**: Settings MUST present a dedicated CSV category, structured like the existing Ollama-models category.
- **FR-017**: Users MUST be able to create, edit, and delete stock presets.
- **FR-018**: A preset MUST have a display name (shown in Settings) and a stock identifier.
- **FR-019**: The stock identifier MUST be unique across presets; duplicates MUST be rejected at configuration time.
- **FR-020**: The stock identifier MUST be usable as a file name; identifiers with illegal file-name characters MUST be rejected with an explanation.
- **FR-021**: A preset MUST hold an ordered list of fields; users MUST be able to add and remove fields.
- **FR-022**: Each field MUST have a CSV column identifier and an app value type.
- **FR-023**: The available app value types MUST be exactly: none, file name, title, description, keywords, category, release filename, editorial, mature content, illustration.
- **FR-024**: A default-value (string) input MUST be available **only** for fields of type "none" and MUST be hidden for all other types.
- **FR-025**: A bool-format choice (yes/no vs true/false) MUST be available **only** for the bool types (editorial, mature content, illustration) and MUST be hidden for all other types.
- **FR-026**: Users MUST be able to reorder a preset's fields, by arrow controls and/or drag-and-drop.
- **FR-027**: Presets and their field lists MUST persist across application sessions.
- **FR-034**: Each preset MUST let the user choose its CSV column delimiter — comma, semicolon, or tab — defaulting to comma.
- **FR-036**: A preset MUST have at least one field; saving a preset with an empty field list MUST be rejected at configuration time with an explanation.

#### Data & feedback

- **FR-028**: All exported metadata values (title, description, keywords, category, and the three flags) MUST come from the photo's database record.
- **FR-029**: If no presets are configured, the export MUST do nothing except inform the user there is nothing to export.
- **FR-030**: If the export scope is empty, the export MUST inform the user there is nothing to export.
- **FR-031**: The export MUST report its outcome to the user (e.g., how many files were written, how many photos exported, and how many — if any — were skipped for lacking a database record).
- **FR-032**: Errors during export (e.g., the destination folder is not writable) MUST be surfaced to the user and logged, without crashing the application.
- **FR-035**: In-scope photos without a database record MUST be excluded from every CSV (not exported with blank or file-derived values), and their count MUST be reported in the export outcome.

### Key Entities *(include if feature involves data)*

- **Stock CSV Preset**: One photo-stock's export configuration. Attributes: display name, stock identifier (unique, file-name-safe), column delimiter (comma/semicolon/tab, default comma), ordered list of CSV Fields. Persisted across sessions.
- **CSV Field**: One column in a preset. Attributes: CSV column identifier (header text), app value type, and — conditionally — a default string value (only for type "none") and a bool-format choice (only for bool types). Carries an order position within its preset.
- **App Value Type**: The fixed set of sources a column can draw from: none, file name, title, description, keywords, category, editorial, mature content, illustration.
- **Export Job**: A single export run. Attributes: scope (selected photos vs. current folder), destination folder, and the set of presets to emit. Produces one CSV file per preset.
- **Photo Metadata Record** (existing, from the SQLite metadata store): the per-photo data the export reads — title, description, keywords, category, editorial/mature-content/illustration flags, and the photo's file name/path.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can create a new stock preset with at least five fields of mixed types and arrange their order in under two minutes.
- **SC-002**: Exporting a folder of N in-scope photos with M configured presets produces exactly M CSV files, each with one header row and one data row for 100% of in-scope photos that have a database record.
- **SC-003**: 100% of exported cell values for app-sourced columns match the corresponding values in the database (no value is derived from the photo file).
- **SC-004**: Bool columns render exactly as configured (yes/no or true/false) in 100% of rows.
- **SC-005**: "none" columns contain the configured default value in 100% of rows.
- **SC-006**: Duplicate or file-name-invalid stock identifiers are rejected 100% of the time at configuration.
- **SC-007**: Every produced CSV opens without parse errors in standard spreadsheet software, including rows whose values contain delimiters, quotes, or line breaks.
- **SC-008**: Exporting 1,000 photos across three presets completes in under five seconds on a typical machine and keeps the UI responsive.

## Assumptions

- **CSV format**: UTF-8 encoding and RFC 4180 double-quote quoting/escaping are global, and a header row is always written. The column delimiter is chosen per preset (comma, semicolon, or tab), defaulting to comma.
- **Multi-value serialization**: Keywords and category are joined into one cell with a comma, with the cell quoted when needed. The in-cell comma separator stays the same regardless of the preset's column delimiter.
- **Preset selection at export time**: An export emits a CSV for **every** configured preset; there is no per-run selection of a subset.
- **Photos without a database record**: Excluded from every CSV and counted in the outcome message — not exported with blank values nor read from the file.
- **Overwrite behavior**: Existing files named `<identifier>.csv` in the destination folder are overwritten.
- **File-name value**: The "file name" type emits the photo's file name including its extension (the submitted asset name).
- **Category source**: "category" maps to the app's stored category data (the comma-joined `categories` value from the metadata store).
- **Value-type set**: The nine types the user enumerated plus release filename (added on request); other store fields are not offered as column types.
- **Reuse of existing UI**: The CSV settings category reuses the established Settings patterns (mirroring the Ollama-models category) and existing dialog/confirmation primitives.
- **Data source**: The SQLite metadata store from feature 008 is the single source of truth for exported values.
